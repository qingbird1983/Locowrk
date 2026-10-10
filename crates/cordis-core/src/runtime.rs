//! Runtime：依赖图与反应式重载（DESIGN.md §3.3）。
//!
//! 三张表：`dependents`（反向依赖：提供者 → 依赖它的 Fiber）、
//! `service_providers`（服务键 → 提供者）、`graph`（声明依赖，拓扑的边来源）。
//! 替换 Provider 四步（§3.3）：沿反向依赖链让下游 Unloading（先撤销链再
//! dispose）→ 卸旧 → 装新 → 按 BFS 层序（即拓扑序）重新激活下游。
//! **事件驱动而非轮询**：每次状态迁移发一条 `fiber.<state>` 事件，订阅方
//! 反应；运行时绝不定时扫描（§3.3/§5.7）。
//!
//! 锁纪律（§9.1/§15.4）：所有表内部可变（DashMap），分片锁绝不跨 await；
//! 环路依赖在激活门不可能形成（依赖必须先满足），BFS visited 集合防御
//! 意外环。`FiberScope` 非 Send，本模块各 async 方法的 future 因此非
//! Send——apply/dispose 必须与登记落在同一线程视图内，调用方不得跨线程
//! spawn。
//!
//! 旧 Provider 重载后保留为 `Unloaded`（逻辑卸载 + 物理保留，§5.6 纪律一），
//! 替换失败时下游保持 `Unloaded` 等依赖恢复，重试归编排层。

use std::any::TypeId;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use serde_json::json;

use crate::context::Context;
use crate::error::{CordisError, Result};
use crate::fiber::{FailureKind, Fiber, FiberId, FiberScope, FiberState};
use crate::plugin::Plugin;

/// 拓扑的边来源（§3.3 `graph` 表）：Fiber 激活时声明依赖的服务键。
#[derive(Default)]
pub struct DependencyGraph {
    dependencies: DashMap<FiberId, Vec<TypeId>>,
}

impl DependencyGraph {
    fn record(&self, id: FiberId, deps: &[TypeId]) {
        if deps.is_empty() {
            self.dependencies.remove(&id);
        } else {
            self.dependencies.insert(id, deps.to_vec());
        }
    }

    fn forget(&self, id: FiberId) {
        self.dependencies.remove(&id);
    }
}

/// 依赖图 + Fiber 表 + 反应式重载（§3.3）。方法全部 `&self`（内部可变性），
/// 外层 `Arc<Runtime>` 共享。
pub struct Runtime {
    fibers: DashMap<FiberId, Arc<Fiber>>,
    dependents: DashMap<FiberId, Vec<FiberId>>,
    service_providers: DashMap<TypeId, FiberId>,
    graph: DependencyGraph,
    next_id: AtomicU64,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            fibers: DashMap::new(),
            dependents: DashMap::new(),
            service_providers: DashMap::new(),
            graph: DependencyGraph::default(),
            // HOST(0) 保留给宿主直接注册；插件 Fiber 从 1 起
            next_id: AtomicU64::new(1),
        }
    }

    /// 查询 Fiber 状态（UI 投影的运行态来源，§11.6）。
    ///
    /// **RESERVED（审计 M-22/D6，v2.148 批 D 定性）**：下面四个观测方法是
    /// DESIGN §11.6 明写的「运行态投影」入口，现网两个组装点都还没接
    /// （当前消费方只有本 crate 的生命周期测试）。与 D3 快照簇同一口径
    /// 处理 = 保留并标注，不当死码删；真接 UI 时它们就是入口。
    pub fn state_of(&self, id: FiberId) -> Option<FiberState> {
        self.fibers.get(&id).map(|f| f.value().state())
    }

    /// 某 Fiber 的失败详情（UI 投影用，§11.6）。
    pub fn failure_of(&self, id: FiberId) -> Option<String> {
        self.fibers.get(&id).and_then(|f| f.value().failure())
    }

    /// 某服务键当前的提供者。
    pub fn provider_of(&self, key: TypeId) -> Option<FiberId> {
        self.service_providers.get(&key).map(|e| *e.value())
    }

    /// 直接依赖某 Fiber 的下游列表（反向依赖表快照）。
    pub fn dependents_of(&self, id: FiberId) -> Vec<FiberId> {
        self.dependents
            .get(&id)
            .map(|e| e.value().clone())
            .unwrap_or_default()
    }

    /// 激活插件：依赖门（`inject` 的服务键必须已提供）→ apply（Fiber 作用域内
    /// 注册服务，Effect 自动按 FiberId 打标签）→ Active。依赖缺失即拒绝，
    /// 不留任何登记（§3.3 状态图；事件驱动重试归编排层）。
    pub async fn activate(&self, ctx: &Context, plugin: Box<dyn Plugin>) -> Result<FiberId> {
        let id = FiberId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let fiber = Arc::new(Fiber::new(id, plugin));
        self.check_deps(ctx, fiber.deps())?;
        self.fibers.insert(id, Arc::clone(&fiber));
        self.graph.record(id, fiber.deps());
        self.link_dependents(id, fiber.deps());
        self.run_apply(ctx, &fiber).await?;
        Ok(id)
    }

    /// 卸载：沿反向依赖链让下游先 Unloading（撤销链 → dispose），最后卸自己。
    /// 幂等：Fiber 未注册或已 Unloaded 直接成功（§3.2 幂等纪律）。
    pub async fn deactivate(&self, ctx: &Context, id: FiberId) -> Result<()> {
        let chain = self.dependents_levels(id);
        for level in chain.iter().rev() {
            for dep in level {
                self.unload_one(ctx, *dep).await?;
                self.fibers.remove(dep);
            }
        }
        self.unload_one(ctx, id).await?;
        self.fibers.remove(&id);
        Ok(())
    }

    /// 替换 Provider（§3.3 四步）：1) 反向依赖闭包 2) 下游 Unloading（撤销链）
    /// 3) 卸旧装新 4) 按拓扑序（BFS 层序）重新激活下游。返回新 Fiber 的 id。
    pub async fn reload_provider(
        &self,
        ctx: &Context,
        id: FiberId,
        replacement: Box<dyn Plugin>,
    ) -> Result<FiberId> {
        let chain = self.dependents_levels(id);
        let mut unloaded = Vec::new();
        // 1+2. 深 → 浅卸下游（撤销链在 unload_one 内执行）
        for level in chain.iter().rev() {
            for dep in level {
                if let Some(fiber) = self.unload_one(ctx, *dep).await? {
                    unloaded.push(fiber);
                }
            }
        }
        // 3. 卸旧装新（旧 Fiber 保留为 Unloaded）
        self.unload_one(ctx, id).await?;
        let new_id = self.activate(ctx, replacement).await?;
        // 4. 浅 → 深重激活（层序即拓扑序）
        for fiber in unloaded.into_iter().rev() {
            self.restart(ctx, fiber).await?;
        }
        Ok(new_id)
    }

    // ---- 内部 ----

    fn check_deps(&self, ctx: &Context, deps: &[TypeId]) -> Result<()> {
        for dep in deps {
            if !ctx.has_service(*dep) {
                return Err(CordisError::DependencyMissing(*dep));
            }
        }
        Ok(())
    }

    /// 反向依赖登记：声明依赖的服务键 → 当前提供者（dependents 表）。
    fn link_dependents(&self, id: FiberId, deps: &[TypeId]) {
        for dep in deps {
            if let Some(provider) = self.service_providers.get(dep) {
                self.dependents.entry(*provider).or_default().push(id);
            }
        }
    }

    /// apply 公共段：Loading →（Fiber 作用域内 apply → 登记提供的服务键）→
    /// Active。失败：先撤销已登记 Effect（§3.2），摘图边，落 Failed。
    async fn run_apply(&self, ctx: &Context, fiber: &Fiber) -> Result<()> {
        let id = fiber.id();
        self.set_state(ctx, fiber, FiberState::Loading).await?;
        let outcome = {
            let _scope = FiberScope::enter(id);
            fiber.plugin().apply(ctx).await
        };
        match outcome {
            Ok(()) => {
                for key in ctx.provided_keys_of(id) {
                    self.service_providers.insert(key, id);
                }
                self.set_state(ctx, fiber, FiberState::Active).await?;
                Ok(())
            }
            Err(e) => {
                ctx.revoke_by_fiber(id).await;
                self.forget_fiber(id);
                fiber.set_failure(e.to_string());
                self.set_state(ctx, fiber, FiberState::Failed(FailureKind::ApplyRejected))
                    .await?;
                Err(CordisError::from(e))
            }
        }
    }

    /// 重激活：Unloaded → Pending（依赖恢复，§3.3 状态图）→ apply 公共段。
    async fn restart(&self, ctx: &Context, fiber: Arc<Fiber>) -> Result<()> {
        self.set_state(ctx, &fiber, FiberState::Pending).await?;
        self.check_deps(ctx, fiber.deps())?;
        self.link_dependents(fiber.id(), fiber.deps());
        self.run_apply(ctx, &fiber).await
    }

    /// 卸载单个 Fiber：Active → Unloading →（撤销链 → dispose）→ Unloaded。
    /// 返回 Some(fiber) 供 reload 重激活；非 Active 入口会被状态机拒绝
    /// （§15.4：不静默降级）；已 Unloaded 视为幂等成功返回 None。
    async fn unload_one(&self, ctx: &Context, id: FiberId) -> Result<Option<Arc<Fiber>>> {
        let Some(fiber) = self.fibers.get(&id).map(|f| Arc::clone(f.value())) else {
            return Ok(None);
        };
        if fiber.state() == FiberState::Unloaded {
            return Ok(None);
        }
        self.set_state(ctx, &fiber, FiberState::Unloading).await?;
        // 撤销链（账本逆序，§3.2）先于 dispose（§3.3 流程第 2 步）
        ctx.revoke_by_fiber(id).await;
        let outcome = {
            let _scope = FiberScope::enter(id);
            fiber.plugin().dispose(ctx).await
        };
        match outcome {
            Ok(()) => {
                self.set_state(ctx, &fiber, FiberState::Unloaded).await?;
                self.forget_fiber(id);
                Ok(Some(fiber))
            }
            Err(e) => {
                fiber.set_failure(e.to_string());
                self.set_state(
                    ctx,
                    &fiber,
                    FiberState::Failed(FailureKind::DisposeRejected),
                )
                .await?;
                Err(CordisError::from(e))
            }
        }
    }

    /// 摘除某 Fiber 的全部图边。服务表仅在仍指向它时移除——可能已被
    /// 新 Provider 接管（§3.1 覆盖语义），不能误删别人的注册。
    fn forget_fiber(&self, id: FiberId) {
        self.graph.forget(id);
        self.service_providers.retain(|_, provider| *provider != id);
        self.dependents.remove(&id);
        self.dependents.retain(|_, list| {
            list.retain(|d| *d != id);
            !list.is_empty()
        });
    }

    /// 反向依赖闭包的 BFS 层序（不含自身；层序即拓扑序，浅层在前）。
    /// visited 集合防御意外环（§15.4），最短路径定层。
    fn dependents_levels(&self, id: FiberId) -> Vec<Vec<FiberId>> {
        let mut levels = Vec::new();
        let mut seen = HashSet::new();
        seen.insert(id);
        let mut frontier = vec![id];
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for cur in &frontier {
                if let Some(list) = self.dependents.get(cur) {
                    for dep in list.value() {
                        if seen.insert(*dep) {
                            next.push(*dep);
                        }
                    }
                }
            }
            if !next.is_empty() {
                levels.push(next.clone());
            }
            frontier = next;
        }
        levels
    }

    /// 状态迁移 + 生命周期事件（§3.3：依赖变化是事件不是轮询）。
    async fn set_state(&self, ctx: &Context, fiber: &Fiber, to: FiberState) -> Result<()> {
        fiber.transition(to)?;
        let mut payload = json!({ "fiber": fiber.id().0, "state": state_name(to) });
        if let Some(message) = fiber.failure() {
            payload["failure"] = json!(message);
        }
        ctx.events
            .emit(&format!("fiber.{}", state_name(to)), &payload)
            .await;
        Ok(())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

fn state_name(state: FiberState) -> &'static str {
    match state {
        FiberState::Pending => "pending",
        FiberState::Loading => "loading",
        FiberState::Active => "active",
        FiberState::Unloading => "unloading",
        FiberState::Unloaded => "unloaded",
        FiberState::Failed(_) => "failed",
    }
}

#[cfg(test)]
mod tests;
