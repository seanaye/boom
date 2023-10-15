import { invoke } from "@tauri-apps/api";
import { Show, createEffect, createSignal } from "solid-js";
import { AUDIO_BUFFER_SIZE } from "../const";
import { useAppContext } from "../Context";

const buffer = new Float32Array(AUDIO_BUFFER_SIZE);
export function PeakRmsMeter() {
  const ctx = useAppContext();
  const [rmsValue, setRmsValue] = createSignal(0);

  return (
    <Show when={ctx.audioStream()}>
      {(audio) => {
        createEffect(() => {
          let animationFrameId: number;
          async function loop() {
            audio().analyzer.getFloatTimeDomainData(buffer);
            const rms: number = await invoke("get_rms", buffer);
            setRmsValue(rms);

            animationFrameId = requestAnimationFrame(loop);
          }
          loop();
          return () => cancelAnimationFrame(animationFrameId);
        });
        return (
          <>
            <meter min="-100" max="10" value={rmsValue()} />
            <input
              oninput={(v) => {
                const value = Number(v.currentTarget.value);
                if (isNaN(value)) return;
                const gainNode = audio().gainNode;
                gainNode.gain.value = value;
              }}
              type="range"
              min="0"
              max="100"
              value="1"
              step="0.01"
            />
          </>
        );
      }}
    </Show>
  );
}
