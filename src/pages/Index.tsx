import { Uploads } from "../components/Uploads";
import { S3ConfigFormList } from "../components/ConfigFormList";
import { SelectDevices } from "../components/SelectDevices";
import { RecordControls } from "../components/RecordControls";
import { PeakRmsMeter } from "../components/PeakMeter";
import { Provider } from "../Context";

function App() {
  return (
    <Provider>
      <div class="rounded-xl p-4 bg-amber-100 text-zinc-600">
        <div class="">
          <div>
            {/* <S3ConfigFormList /> */}
            <RecordControls />
            <SelectDevices />
            {/* <CameraPreview /> */}
            <PeakRmsMeter />
            <Uploads />
          </div>
        </div>
      </div>
    </Provider>
  );
}

export default App;
