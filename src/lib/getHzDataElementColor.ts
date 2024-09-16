import type { ColoringMethod } from "./fft";

const interpolate = (
  value: number,
  { inputMin = 0, inputMax = 1, outputMin = 0, outputMax = 1 }
) => {
  const adjustedValue = value - inputMin;
  const adjustedMax = inputMax - inputMin;
  const ratio = adjustedValue / adjustedMax;

  const output = ratio * (outputMax - outputMin) + outputMin;

  return output;
};

// 0 to 255 in
// 0 to 255 out
const sigmoid = (x: number) => {
  const a = 9;
  const b = -0.05;

  const numerator = 255;

  const power = a + b * x;
  const denominator = 1 + Math.pow(Math.E, power);

  return numerator / denominator;
};

export function getAudioAmplitudeValueColor(
  _hz: number,
  value: number,
  coloring: ColoringMethod = "sigmoid"
) {
  const colorMax = 255;
  const constainedValue = interpolate(value, {
    inputMin: 0,
    inputMax: 255,
    outputMin: 0,
    outputMax: 1,
  });

  const colorValue = interpolate(constainedValue, {
    inputMin: 0,
    inputMax: 1,
    outputMin: 0,
    outputMax: 255,
  });

  if (coloring === "sigmoid") {
    const adjustedColorValue = sigmoid(colorValue);
    const color = colorMax - adjustedColorValue;

    const l = `${Math.round(10 * Math.log(adjustedColorValue))}%`; // max l = ~55% with x=255

    return `hsl(${color}, 100%, ${l})`;
  }

  if (colorValue === 0) {
    return "black";
  }

  const color = 255 - colorValue;
  return `hsl(${color}, 100%, ${Math.min(Math.abs(colorValue - 128), 50)}%)`;
}
