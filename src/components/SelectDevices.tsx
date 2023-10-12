import { For, Show } from "solid-js";
import { useAppContext } from "../Context";

export function SelectDevices() {
  const ctx = useAppContext();
  const selectedAudioDevice = () => {
    const a = ctx.audioDevices();
    if (!a || a.length === 0) return null;
    const selectedId = ctx.selectedAudio();
    const out = a.find((k) => k.deviceId === selectedId) ?? a[0];
    return out;
  };
  return (
    <Show when={ctx.audioStream()}>
      <div>
        <label>
          <select
            onChange={(e) => {
              ctx.setSelectedAudio(e.currentTarget.value);
            }}
            value={selectedAudioDevice()?.deviceId}
          >
            <For each={ctx.audioDevices()}>
              {(d) => <option value={d.deviceId}>{d.label}</option>}
            </For>
          </select>
        </label>
        {/* <label> */}
        {/*   <select */}
        {/*     onChange={(e) => { */}
        {/*       setSelectedVideo(e.currentTarget.value); */}
        {/*     }} */}
        {/*   > */}
        {/*     <For each={devices()}> */}
        {/*       {(d) => ( */}
        {/*         <Show when={d.kind === "videoinput"}> */}
        {/*           <option value={d.deviceId}>{d.label}</option> */}
        {/*         </Show> */}
        {/*       )} */}
        {/*     </For> */}
        {/*   </select> */}
        {/* </label> */}
      </div>
    </Show>
  );
}
