import {
  createEffect,
  createResource,
  createSignal,
  onCleanup,
} from "solid-js";
import { invoke } from "@tauri-apps/api/primitives";

export default function Screenshot() {
  let canvasRef: HTMLCanvasElement | undefined;
  const [startX, setStartX] = createSignal<number | null>(null);
  const [startY, setStartY] = createSignal<number | null>(null);
  const [endX, setEndX] = createSignal<number | null>(null);
  const [endY, setEndY] = createSignal<number | null>(null);

  const canStart = () => startX() != null && startY() != null;
  const minX = () => Math.min(startX() ?? 0, endX() ?? 0);
  const minY = () => Math.min(startY() ?? 0, endY() ?? 0);
  const diffX = () => Math.abs((startX() ?? 0) - (endX() ?? 0));
  const diffY = () => Math.abs((startY() ?? 0) - (endY() ?? 0));
  const ctxSig = () => {
    return canvasRef?.getContext("2d") ?? null;
  };

  const [taskbar] = createResource<number>(
    () => invoke("plugin:screenshot|get_taskbar_offset"),
    { initialValue: 0 },
  );

  createEffect(() => {
    if (!canStart()) return;

    const ctx = ctxSig();
    if (!ctx) return;
    const sx = minX();
    const sy = minY();
    const dx = diffX();
    const dy = diffY();

    const frame = requestAnimationFrame(() => {
      ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
      ctx.fillStyle = "rgba(255, 255, 255, 0.5)";
      ctx.fillRect(sx, sy - taskbar(), dx, dy);
    });
    onCleanup(() => cancelAnimationFrame(frame));
  });

  return (
    <div class="w-full h-full">
      <canvas
        ref={canvasRef}
        onMouseDown={(e) => {
          setStartX(e.screenX);
          setStartY(e.screenY);
        }}
        width={window.innerWidth}
        height={window.innerHeight}
        onMouseMove={(e) => {
          // invoke("plugin:screenshot|debug", {
          //   s: JSON.stringify({
          //     sx: startX(),
          //     sy: startY(),
          //     ex: endX(),
          //     ey: endY(),
          //   }),
          // });
          // if (!canStart()) return;
          setEndX(e.screenX);
          setEndY(e.screenY);
        }}
        onMouseUp={() => {
          invoke("plugin:screenshot|finish_screenshot", {
            pointA: { x: startX(), y: startY() },
            pointB: { x: endX(), y: endY() },
          });
        }}
        class="w-full h-full"
      />
    </div>
  );
}
