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
        await invoke("plugin:api|list_uploads");
      return r;
    },
  );
  return (
    <div class="grid grid-flow-row grid-cols-6 gap-4">
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
    console.log("deleting", props.id);
    await invoke("plugin:api|delete_upload", { id: props.id });
    console.log("after");
    return props.refetch();
  });
  const [copySignal, setCopySignal] = createSignal<string | null>(null);
  const [copied] = createResource(copySignal, async (copy) => {
    await writeText(copy);
    return new Promise((resolve) => setTimeout(resolve, 1000));
  });
  return (
    <>
      <div class="col-span-4">
        {new Date(props.created_at).toLocaleString()}
      </div>
      {/* <div>{(deletion.error as Error)?.message}</div> */}
      <button
        onclick={() => setDelSignal(true)}
        disabled={deletion.loading}
        class="rounded bg-gray-100 p-1 shadow mx-auto"
      >
        <Show when={deletion.loading} fallback="delete">
          ...
        </Show>
      </button>
      <button
        onclick={() =>
          setCopySignal(`https://vidview.deno.dev/?v=${props.url}`)
        }
        disabled={copied.loading}
        class="rounded bg-gray-100 p-1 shadow mx-auto text-zinc-500"
      >
        <Show
          when={copied.loading}
          fallback={<div class="i-heroicons-clipboard-document-20-solid" />}
        >
          <div class="i-heroicons-clipboard-document-check-20-solid" />
        </Show>
      </button>
    </>
  );
}
