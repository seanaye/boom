import { Uploads } from "../components/Uploads";
import { SelectDevices } from "../components/SelectDevices";
import { RecordControls } from "../components/RecordControls";
import { PeakRmsMeter } from "../components/PeakMeter";
import { A } from "@solidjs/router";
import Layout from "../components/Layout";

function App() {
  return (
    <Layout>
      <div class="flex flex-row justify-between">
        <RecordControls />
        <A
          class="i-heroicons-cog-6-tooth h-6 w-6 hover:text-zinc-900"
          href="/settings"
        />
      </div>
      <SelectDevices />
      {/* <CameraPreview /> */}
      <PeakRmsMeter />
      <Uploads />
    </Layout>
  );
}

export default App;
