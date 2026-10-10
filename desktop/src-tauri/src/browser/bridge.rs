//! 预览页 console 桥（v2.144）：注入进 lwpreview 服务的 text/html 响应，
//! 把 console.error/warn、window error、unhandledrejection 以 postMessage
//! 转给父页（工作面板浏览器页签收集，经 `browser_report_errors` 落
//! ConsoleStore）。只读观测面：不改页面行为，原样放行 console。

/// 注入的桥接脚本（IIFE，幂等；文本钳 2000 字符防巨型报文）。
pub const BRIDGE_JS: &str = r#"(function(){
  if (window.__lwPreviewBridge) return; window.__lwPreviewBridge = true;
  var CLIP = 2000;
  function fmt(args){
    var out = [];
    for (var i = 0; i < args.length && i < 6; i++) {
      var v = args[i];
      try { out.push(typeof v === 'string' ? v : JSON.stringify(v)); }
      catch (e) { try { out.push(String(v)); } catch (_) { out.push('[unserializable]'); } }
    }
    var s = out.join(' ');
    return s.length > CLIP ? s.slice(0, CLIP) : s;
  }
  function send(level, text){
    try { window.parent.postMessage({ __lwPreview: true, level: level, text: text, ts: Date.now() }, '*'); } catch (e) {}
  }
  ['error', 'warn'].forEach(function (k) {
    var orig = console[k] ? console[k].bind(console) : function () {};
    console[k] = function () { send(k, fmt(arguments)); orig.apply(null, arguments); };
  });
  window.addEventListener('error', function (e) {
    var loc = e.filename ? ' (' + e.filename.split('/').pop() + ':' + e.lineno + ')' : '';
    send('error', (e.message || 'script error') + loc);
  });
  window.addEventListener('unhandledrejection', function (e) {
    var r = e.reason, t;
    try { t = typeof r === 'string' ? r : (r && r.message) || JSON.stringify(r); }
    catch (_) { t = String(r); }
    send('error', 'Unhandled rejection: ' + t);
  });
})();"#;

/// 注入点：优先 `<head…>` 之后（保 doctype 不落怪异模式），退 `<html…>`
/// 之后，都没有就整体前置（碎片页无所谓模式）。
pub fn inject_bridge(html: &str) -> String {
    let script = format!("<script>{BRIDGE_JS}</script>");
    let lower = html.to_ascii_lowercase();
    for open in ["<head", "<html"] {
        if let Some(pos) = lower.find(open) {
            if let Some(gt) = lower[pos..].find('>') {
                let at = pos + gt + 1;
                let mut out = String::with_capacity(html.len() + script.len());
                out.push_str(&html[..at]);
                out.push_str(&script);
                out.push_str(&html[at..]);
                return out;
            }
        }
    }
    format!("{script}{html}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_after_head_tag() {
        let page = "<!DOCTYPE html>\n<html lang=\"zh\"><head>\n<meta charset=\"utf-8\">\n<title>t</title>\n</head><body></body></html>";
        let out = inject_bridge(page);
        assert!(out.contains("<!DOCTYPE html>"), "doctype 必须原样在最前");
        let head_end = out.find("<head>").expect("head tag kept") + "<head>".len();
        assert!(
            out[head_end..].starts_with("<script>"),
            "桥接脚本紧跟 <head> 之后"
        );
        assert!(out.contains("__lwPreviewBridge"));
    }

    #[test]
    fn injects_after_html_tag_when_no_head() {
        let page = "<html><body>hi</body></html>";
        let out = inject_bridge(page);
        assert!(out.starts_with("<html><script>"));
    }

    #[test]
    fn prepends_when_no_structure() {
        let page = "<p>fragment</p>";
        let out = inject_bridge(page);
        assert!(out.starts_with("<script>"));
        assert!(out.ends_with("<p>fragment</p>"));
    }

    #[test]
    fn bridge_is_idempotent_shaped() {
        assert!(BRIDGE_JS.contains("if (window.__lwPreviewBridge) return"));
    }
}
