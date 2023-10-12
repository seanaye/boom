import { Show } from "solid-js";
import { useAppContext } from "../Context";

export function RecordControls() {
  const context = useAppContext();
  return (
    <Show
      when={context.finalMediaStream()}
      fallback={
        <button
          onclick={() => context.requestPermissions()}
          disabled={context.displayPermission.loading}
        >
          <Show
            when={context.displayPermission.loading}
            fallback="request permission"
          >
            ...
          </Show>
        </button>
      }
    >
      <Show
        when={context.isRecording()}
        fallback={
          <button type="button" onClick={context.startRecording}>
            Record
          </button>
        }
      >
        <button type="button" onClick={context.stopRecording}>
          Stop
        </button>
      </Show>
    </Show>
  );
}
