import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => {
  cleanup();
  window.location.hash = "";
});

// jsdom has no layout. React Flow needs these to render nodes in tests; they
// follow React Flow's own testing guide.
class ResizeObserverStub {
  constructor(private callback: ResizeObserverCallback) {}
  observe(target: Element) {
    const contentRect = { x: 0, y: 0, width: 1000, height: 800, top: 0, left: 0, right: 1000, bottom: 800 } as DOMRectReadOnly;
    this.callback([{ target, contentRect } as ResizeObserverEntry], this as unknown as ResizeObserver);
  }
  unobserve() {}
  disconnect() {}
}
class DOMMatrixReadOnlyStub {
  m22: number;
  constructor(transform?: string) {
    const scale = transform?.match(/scale\(([1-9.])\)/)?.[1];
    this.m22 = scale !== undefined ? Number(scale) : 1;
  }
}
globalThis.ResizeObserver ??= ResizeObserverStub as unknown as typeof ResizeObserver;
(globalThis as Record<string, unknown>).DOMMatrixReadOnly ??= DOMMatrixReadOnlyStub;
Object.defineProperties(HTMLElement.prototype, {
  offsetHeight: { get() { return parseFloat(this.style.height) || 1; } },
  offsetWidth: { get() { return parseFloat(this.style.width) || 1; } },
});
(SVGElement.prototype as unknown as { getBBox: () => DOMRect }).getBBox = () => ({ x: 0, y: 0, width: 0, height: 0 }) as DOMRect;

// jsdom has no PointerEvent; a MouseEvent carries the coordinates the tests need.
if (!("PointerEvent" in globalThis)) {
  (globalThis as Record<string, unknown>).PointerEvent = class PointerEvent extends MouseEvent {
    pointerId: number;
    constructor(type: string, init: PointerEventInit = {}) {
      super(type, init);
      this.pointerId = init.pointerId ?? 1;
    }
  };
}
