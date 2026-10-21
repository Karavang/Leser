import presetEnv from "postcss-preset-env";

export default {
  plugins: [presetEnv({ features: { "custom-properties": { preserve: true } } })],
};
