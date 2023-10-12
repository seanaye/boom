import { defineConfig, presetIcons } from "unocss";
import { presetUno } from "unocss";

export default defineConfig({
  presets: [
    presetUno(),
    presetIcons()
  ],
});
