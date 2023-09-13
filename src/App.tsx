import { Accessor, Show, createResource, createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { invoke, Channel } from "@tauri-apps/api/tauri";

function App() {
  const [recording, setRecording] = createSignal<MediaRecorder | null>(null);

  const mimes = [
    "video/webm; codecs=vp9",
    "video/webm",
    "video/mpeg",
    "video/mp4",
  ];

  const mime = mimes.find(MediaRecorder.isTypeSupported);

  if (!mime) {
    return <div>platform not supported</div>;
  }

  const [{ loading: streaming, error }] = createResource(
    recording,
    async (mediaRecorder) => {
      const body = new ReadableStream({
        start: (controller) => {
          mediaRecorder.ondataavailable = async (e) => {
            controller.enqueue(new Uint8Array(await e.data.arrayBuffer()));
          };
          mediaRecorder.onstop = () => {
            console.log("onstop");
            controller.close();
          };

          mediaRecorder.start(1000);
        },
        // pull: () => {
        //   console.log('pull', mediaRecorder)
        //   setTimeout(() => mediaRecorder.requestData(), 10)
        // },
        cancel: () => {
          console.log("cancel");
          mediaRecorder.stop();
        },
      });
      const out = fetch(
        "https://06b9-2607-fea8-1c40-d400-18d7-1579-b697-8c5e.ngrok-free.app/api/stream",
        {
          method: "POST",
          body,
          headers: { "Content-Type": mime },
          // @ts-ignore
          duplex: "half",
        },
      );
      console.log(out);
      console.log(await out);
      return out;
    },
  );

  async function record() {
    const stream = await navigator.mediaDevices.getDisplayMedia({
      video: true,
    });

    const mediaRecorder = new MediaRecorder(stream, {
      mimeType: mime,
    });
    setRecording(mediaRecorder);
  }

  function stop() {
    const recorder = recording();
    if (!recorder) return;
    recorder.stop();
    setRecording(null);
  }

  return (
    <div class="">
      <div class="">
        <div>
          <button type="button" onClick={record}>
            Record
          </button>
          <button type="button" onClick={stop}>
            Stop
          </button>
          <p>{error}</p>
          <Show when={streaming}>
            <div>streaming...</div>
          </Show>
          <S3ConfigFormList />
          <CreateS3ConfigForm />
        </div>
      </div>
    </div>
  );
}

function CreateS3ConfigForm() {
  const [form, setForm] = createStore({
    nickname: "",
    endpoint: "",
    region: "",
    bucket_name: "",
    host_rewrite: "",
    public_key: "",
    private_key: "",
  });

  const updateFormField = (fieldName: string) => (event: Event) => {
    const inputElement = event.currentTarget as HTMLInputElement;
    setForm({
      [fieldName]: inputElement.value,
    });
  };
  return (
    <form
      class="grid flow-col gap-4"
      onSubmit={async (e) => {
        e.preventDefault();
        const config = JSON.parse(JSON.stringify(form));
        console.log({ d: config });
        const res = await invoke("plugin:api|create_config", { config });
        console.log(res);
      }}
    >
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("nickname")}
        />
        Nickname
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("endpoint")}
        />
        Endpoint
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("region")}
        />
        Region
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("bucket_name")}
        />
        Bucket Name
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("host_rewrite")}
        />
        Host Rewrite (Optional)
      </label>
      <label>
        <input
          type="text"
          placeholder="name"
          onChange={updateFormField("public_key")}
        />
        Public Key
      </label>
      <label>
        <input
          type="password"
          placeholder="name"
          onChange={updateFormField("private_key")}
        />
        Private key
      </label>
      <button type="submit">Create</button>
    </form>
  );
}

function S3ConfigFormList() {
  const [formData] = createResource(async () => {
    const r = await invoke("plugin:api|list_configs");
    console.log(r);
    return r;
  });
  return <div>{JSON.stringify(formData())}</div>;
}

export default App;
