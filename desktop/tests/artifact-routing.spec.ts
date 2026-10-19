import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { extOf, routeArtifact } from "../src/lib/artifact-routing";
import { useWorkPanelStore } from "../src/stores/work-panel";

describe("artifact-routing", () => {
  it("routes pages to browser, images to image, everything else to file", () => {
    expect(routeArtifact("/w/index.html")).toBe("browser");
    expect(routeArtifact("/w/INDEX.HTM")).toBe("browser");
    expect(routeArtifact("C:\\w\\shot.PNG")).toBe("image");
    expect(routeArtifact("/w/logo.svg")).toBe("image");
    expect(routeArtifact("/w/notes.md")).toBe("file");
    expect(routeArtifact("/w/src/main.ts")).toBe("file");
  });

  it("takes the extension from the last segment only", () => {
    expect(extOf("C:\\w\\v1.2\\README")).toBe("");
    expect(routeArtifact("C:\\w\\v1.2\\README")).toBe("file");
    expect(extOf("C:\\w\\a.md\\b")).toBe("");
    expect(routeArtifact("C:\\w\\.gitignore")).toBe("file");
  });
});

describe("work panel artifact tabs", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("keeps one file tab per path and moves the target range", () => {
    const wp = useWorkPanelStore();
    wp.openFileTab("/w/a.ts", 3, 3);
    wp.openFileTab("/w/a.ts", 12, 12);
    const files = wp.tabs.filter((t) => t.kind === "file");
    expect(files).toHaveLength(1);
    expect(files[0]?.line).toBe(12);
    expect(files[0]?.endLine).toBe(12);
    expect(wp.activeTabId).toBe("file:/w/a.ts");
  });

  it("keeps one image tab per asset", () => {
    const wp = useWorkPanelStore();
    wp.openImageTab("shot.png", "a1", "image/png");
    wp.openImageTab("shot.png", "a1", "image/png");
    const images = wp.tabs.filter((t) => t.kind === "image");
    expect(images).toHaveLength(1);
    expect(images[0]?.assetId).toBe("a1");
    expect(wp.activeTabId).toBe("img:a1");
  });
});
