import * as zod from "zod";

export interface Settings {
  appearance: Appearance;
  coco: CoCo;
  featureFlags: FeatureFlags;
}

export interface FeatureFlags {
  funding: boolean;
}

export interface Appearance {
  theme: Theme;
  hints: {
    showRemoteHelper: boolean;
  };
}

export interface CoCo {
  seeds: string[];
}

export enum Theme {
  Dark = "dark",
  Light = "light",
}

export const settingsSchema: zod.ZodSchema<Settings> = zod.object({
  appearance: zod.object({
    hints: zod.object({
      showRemoteHelper: zod.boolean(),
    }),
    theme: zod.enum([Theme.Dark, Theme.Light]),
  }),
  coco: zod.object({
    seeds: zod.array(zod.string()),
  }),
  featureFlags: zod.object({
    funding: zod.boolean(),
  }),
});
