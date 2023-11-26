import { invoke } from "@tauri-apps/api/primitives";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  createEffect,
  createResource,
  createSignal,
  For,
  Show,
} from "solid-js";
import { useAppContext } from "../Context";

export function Uploads() {
  const ctx = useAppContext();
  const [uploads, { refetch }] = createResource(
    () => !ctx.isRecording(),
    async () => {
      console.log("fetching");
      const r: Array<{
        id: number;
        url: string;
        created_at: string;
        mime_type: string;
      }> = await invoke("list_uploads");
      console.log(r);
      return r;
    },
  );

  const r = () => {
    console.log("refetch from backend");
    refetch();
  };

  createEffect(() => {
    return listen("reload-uploads", r);
  });
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
  mime_type: string;
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
  const datestr = new Date(props.created_at).toLocaleString();
  return (
    <details>
      <summary class="grid grid-cols-7 py-2 items-center">
        <div class="col-span-4 flex flex-row items-center gap-2">
          <Icon mime_type={props.mime_type} />
          {datestr}
        </div>
        {/* <div>{(deletion.error as Error)?.message}</div> */}
        <div class="mx-auto col-span-1">
          <button
            onclick={() => setDelSignal(true)}
            disabled={deletion.loading}
            class="rounded bg-gray-100 p-2 shadow mx-auto"
          >
            <Show
              when={deletion.loading}
              fallback={<div class="i-heroicons-trash-20-solid" />}
            >
              ...
            </Show>
          </button>
        </div>
        <div class="mx-auto col-span-1">
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
      </summary>
      <Media mime_type={props.mime_type} src={props.url} />
    </details>
  );
}

function Icon(props: { mime_type: string }) {
  return (
    <Show
      when={props.mime_type.includes("video")}
      fallback={<div class="i-heroicons-photo-20-solid" />}
    >
      <div class="i-heroicons-video-camera-20-solid" />
    </Show>
  );
}

function Media(props: { mime_type: string; src: string }) {
  return (
    <Show
      when={props.mime_type.includes("video")}
      fallback={<img src={props.src} loading="lazy" decoding="async" />}
    >
      <video src={props.src} controls />
    </Show>
  );
}
