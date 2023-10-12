import {
  Accessor,
  JSX,
  Resource,
  createContext,
  createEffect,
  createResource,
  createSignal,
  useContext,
} from "solid-js";
import { invoke } from "@tauri-apps/api";
import { AUDIO_BUFFER_SIZE } from "./const";

function createMediaRecorderPromise(mediaRecorder: MediaRecorder) {
  const promise = new Promise<void>((resolve, reject) => {
    mediaRecorder.ondataavailable = async (e) => {
      const buf = await e.data.arrayBuffer();
      const options =
        mediaRecorder.state !== "recording"
          ? { headers: { final: "true" } }
          : undefined;
      invoke("plugin:api|upload_url_part", buf, options).then((v) => {
        console.log(v);
        if (v) resolve();
      }, reject);
    };
  });

  mediaRecorder.start(1000);
  return promise;
}

type AudioStreamData = {
  stream: MediaStream;
  analyzer: AnalyserNode;
  gainNode: GainNode;
};

type Context = {
  audioDevices: Accessor<Array<MediaDeviceInfo>>;
  displayPermission: Resource<MediaStream>;
  finalMediaStream: Accessor<MediaStream | null>;
  stopRecording: () => void;
  startRecording: () => void;
  requestPermissions: () => void;
  reloadAudioStream: () => void;
  isRecording: Accessor<boolean>;
  selectedAudio: Accessor<null | string>;
  setSelectedAudio: (id: string) => void;
  audioStream: Resource<AudioStreamData>;
};
const AppContext = createContext<Context | null>();

export function Provider(props: { children: JSX.Element }) {
  const [getMedia, setGetMedia] = createSignal(0);
  function requestPermissions() {
    setGetMedia((c) => c + 1);
  }

  const [displayPermission] = createResource(
    () => getMedia() > 0,
    async () => {
      const a = performance.now();
      const out = await navigator.mediaDevices.getDisplayMedia({
        video: true,
        audio: true,
      });
      console.log(`getDisplayMedia took ${performance.now() - a}`);
      return out;
    },
  );

  const mediaStream = () => {
    const tracks = audioStream()?.stream?.getAudioTracks();
    const audio = tracks?.at(0);
    const video = displayPermission()?.getVideoTracks().at(0);
    const displayAudio = displayPermission()?.getAudioTracks().at(0);
    console.log(displayAudio);
    if (!audio || !video) return null;
    const newStream = new MediaStream([audio, video]);
    if (displayAudio) newStream.addTrack(displayAudio);
    return newStream;
  };

  const [recorder, setRecorder] = createSignal<MediaRecorder | null>(null);
  function startRecording() {
    const stream = mediaStream();
    if (!stream) return console.log("stream not ready");
    return setRecorder(() => new MediaRecorder(stream));
  }

  function stopRecording() {
    const rec = recorder();
    rec?.stop();
  }

  const [resource] = createResource(recorder, async (recorder) => {
    console.log(recorder);
    await invoke("plugin:api|begin_upload");
    return createMediaRecorderPromise(recorder);
  });
  const isRecording = () => resource.loading;

  const [selectedAudio, setSelectedAudio] = createSignal<null | string>(null);
  const selectAudioContraints = (): MediaTrackConstraints | true => {
    const selected = selectedAudio();
    if (!selected) return true;
    return { deviceId: { exact: selected } };
  };

  const [audioStream, { refetch: reloadAudioStream }] = createResource(
    () => {
      if (getMedia() === 0) return null;
      return {
        contraints: selectAudioContraints(),
        display: displayPermission(),
      };
    },
    async (audioSelection) => {
      const a = performance.now();
      const source = await navigator.mediaDevices.getUserMedia({
        audio: audioSelection.contraints,
      });
      console.log(`getUserMedia took ${performance.now() - a}`);

      const audioCtx = new AudioContext();
      const gainNode = audioCtx.createGain();
      const sourceNode = audioCtx.createMediaStreamSource(source);
      const destNode = audioCtx.createMediaStreamDestination();
      const analyzer = audioCtx.createAnalyser();

      analyzer.fftSize = AUDIO_BUFFER_SIZE;

      const audioTracks = audioSelection.display?.getAudioTracks();
      console.log(audioTracks)
      if (audioTracks?.length && audioSelection.display) {
        const displaySource = audioCtx.createMediaStreamSource(
          audioSelection.display,
        );
        displaySource.connect(destNode);
      }
      sourceNode.connect(gainNode);
      gainNode.connect(analyzer);
      gainNode.connect(destNode);

      return { stream: destNode.stream, analyzer, gainNode };
    },
  );

  const [devices, { refetch: refetchAudioDevices }] = createResource(
    audioStream,
    () => navigator.mediaDevices.enumerateDevices(),
  );

  createEffect(() => {
    navigator.mediaDevices.addEventListener(
      "devicechange",
      refetchAudioDevices,
    );
    return () =>
      navigator.mediaDevices.removeEventListener(
        "devicechange",
        refetchAudioDevices,
      );
  });

  const audioDevices = () => {
    return (devices() ?? []).filter((k) => k.kind === "audioinput");
  };

  return (
    <AppContext.Provider
      value={{
        finalMediaStream: mediaStream,
        audioDevices,
        audioStream,
        stopRecording,
        startRecording,
        requestPermissions,
        reloadAudioStream,
        isRecording,
        selectedAudio,
        setSelectedAudio,
        displayPermission,
      }}
    >
      {props.children}
    </AppContext.Provider>
  );
}

export function useAppContext() {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("No context found");
  return ctx;
}
