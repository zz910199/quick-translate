import { useEffect, useRef, useState, type PointerEvent } from "react";
import type { CapturedDisplayDto, ScreenshotRect } from "../../lib/commands";

interface Point { x: number; y: number }
export function ScreenshotOverlay({ display, onSelect, onCancel }: {
  display: CapturedDisplayDto;
  onSelect: (displayId: string, rect: ScreenshotRect) => void;
  onCancel: () => void;
}) {
  const origin = useRef<Point | null>(null);
  const [drag, setDrag] = useState<{ start: Point; end: Point } | null>(null);
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => { if (event.key === "Escape") onCancel(); };
    window.addEventListener("keydown", keydown);
    return () => window.removeEventListener("keydown", keydown);
  }, [onCancel]);
  function point(event: PointerEvent<HTMLImageElement>): Point {
    const bounds = event.currentTarget.getBoundingClientRect();
    return { x: Math.max(0, Math.min(bounds.width, event.clientX - bounds.left)),
      y: Math.max(0, Math.min(bounds.height, event.clientY - bounds.top)) };
  }
  function complete(event: PointerEvent<HTMLImageElement>) {
    if (!origin.current) return;
    const start = origin.current;
    origin.current = null;
    const end = point(event);
    setDrag(null);
    const bounds = event.currentTarget.getBoundingClientRect();
    const scaleX = display.scaleFactor * ((display.width / display.scaleFactor) / bounds.width);
    const scaleY = display.scaleFactor * ((display.height / display.scaleFactor) / bounds.height);
    const left = Math.max(0, Math.min(display.width, Math.floor(Math.min(start.x, end.x) * scaleX)));
    const top = Math.max(0, Math.min(display.height, Math.floor(Math.min(start.y, end.y) * scaleY)));
    const right = Math.max(0, Math.min(display.width, Math.ceil(Math.max(start.x, end.x) * scaleX)));
    const bottom = Math.max(0, Math.min(display.height, Math.ceil(Math.max(start.y, end.y) * scaleY)));
    if (right > left && bottom > top) onSelect(display.id, { x: left, y: top, width: right - left, height: bottom - top });
  }
  return <div className="screenshot-overlay">
    <img className="screenshot-image" role="img" aria-label="屏幕截图" draggable={false} src={display.previewDataUrl}
      onPointerDown={event => { origin.current = point(event); setDrag({ start: origin.current, end: origin.current }); event.currentTarget.setPointerCapture?.(event.pointerId); }}
      onPointerMove={event => { if (origin.current) setDrag({ start: origin.current, end: point(event) }); }}
      onPointerUp={complete} onPointerCancel={() => { origin.current = null; setDrag(null); }} />
    {drag && <div className="screenshot-selection" style={{ left: Math.min(drag.start.x, drag.end.x), top: Math.min(drag.start.y, drag.end.y),
      width: Math.abs(drag.end.x - drag.start.x), height: Math.abs(drag.end.y - drag.start.y) }} />}
    <button type="button" className="screenshot-cancel" onClick={onCancel} aria-label="取消截图">×</button>
  </div>;
}
