import { invoke } from "@tauri-apps/api";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { createResource, createSignal, For, Show } from "solid-js";
import { useAppContext } from "../Context";

export function Uploads() {
  const ctx = useAppContext();
  const [uploads, { refetch }] = createResource(
    () => !ctx.isRecording(),
    async () => {
      console.log("fetching");
      const r: Array<{ id: number; url: string; created_at: string }> =
        await invoke("list_uploads");
      return r;
    },
  );
  return (
    <div class="flex flex-col divide-y-2 border-black">
      <For each={uploads()}>{(d) => <Upload {...d} refetch={refetch} />}</For>
    </div>
  );
}

function Upload(props: {
  id: number;
  url: string;
  created_at: string;
  refetch: () => void;
}) {
  const [delSignal, setDelSignal] = createSignal(false);
  const [deletion] = createResource(delSignal, async () => {
    await invoke("delete_upload", { id: props.id });
    return props.refetch();
  });
  const [copySignal, setCopySignal] = createSignal<string | null>(null);
  const [copied] = createResource(copySignal, async (copy) => {
    await writeText(copy);
    return new Promise((resolve) => setTimeout(resolve, 1000));
  });
  return (
    <div class="grid grid-cols-6 py-2">
      <div class="col-span-4">
        <details>
          <summary>{new Date(props.created_at).toLocaleString()}</summary>
          <video src={props.url} controls />
        </details>
      </div>
      {/* <div>{(deletion.error as Error)?.message}</div> */}
      <div class="mx-auto">
        <button
          onclick={() => setDelSignal(true)}
          disabled={deletion.loading}
          class="rounded bg-gray-100 p-1 shadow mx-auto"
        >
          <Show when={deletion.loading} fallback="delete">
            ...
          </Show>
        </button>
      </div>
      <div class="mx-auto">
        <button
          onclick={() =>
            setCopySignal(`https://vidview.deno.dev/?v=${props.url}`)
          }
          disabled={copied.loading}
          class="rounded bg-gray-100 p-2 shadow mx-auto text-zinc-500"
        >
          <Show
            when={copied.loading}
            fallback={<div class="i-heroicons-clipboard-document-20-solid" />}
          >
            <div class="i-heroicons-clipboard-document-check-20-solid" />
          </Show>
        </button>
      </div>
    </div>
  );
}
