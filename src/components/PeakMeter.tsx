import { invoke } from "@tauri-apps/api/primitives";
import { Show, createEffect, createSignal } from "solid-js";
import { useAppContext } from "../Context";

export function PeakRmsMeter() {
  const ctx = useAppContext();
  const [rmsValue, setRmsValue] = createSignal(0);

  return (
    <Show when={ctx.audioStream()}>
      {(audio) => {
        createEffect(() => {
          let animationFrameId: number;
          async function loop() {
            const buffer = audio().buffer;
            audio().analyzer.getByteFrequencyData(buffer);
            const rms: number = await invoke("get_rms", buffer);
            setRmsValue(rms);

            animationFrameId = requestAnimationFrame(loop);
          }
          loop();
          return () => cancelAnimationFrame(animationFrameId);
        });
        return (
          <>
            <meter max="1" value={rmsValue()} />
            <input
              oninput={(v) => {
                const value = Number(v.currentTarget.value);
                if (isNaN(value)) return;
                const gainNode = audio().gainNode;
                gainNode.gain.value = value;
              }}
              type="range"
              min="0"
              max="10"
              value="1"
              step="0.01"
            />
          </>
        );
      }}
    </Show>
  );
}
