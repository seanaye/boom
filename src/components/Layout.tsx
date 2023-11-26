import { JSX } from "solid-js";
import { Provider } from "../Context";

export default function Layout(props: { children: JSX.Element }) {
  return (
    <Provider>
      <div class="rounded-xl p-4 bg-amber-100 text-zinc-600" style={{"min-height": "600px"}}>
        {props.children}
      </div>
    </Provider>
  );
}
