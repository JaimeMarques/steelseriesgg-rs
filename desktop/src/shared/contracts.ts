import { z } from "zod";
export const mutationTimeout = "Audio service response timed out; change may still complete. Refresh to reconcile current state before retrying.";
export const identifier = z.string().min(1).max(256);
export const numericId = z.string().max(10).regex(/^(0|[1-9][0-9]*)$/).refine((value) => Number(value) <= 4294967295, "ID exceeds u32");
export const gain = z.number().finite().min(0).max(1);
const observedGain = z.number().finite().min(0).max(65536); // Pulse u32 fixed-point volume; writes remain 0..1.
export const inputMode = z.enum(["software", "hardware"]);
export const groupId = z.enum(["game", "chat", "media", "unmanaged"]);
export const side = z.enum(["a", "b", "none"]);
export const profileName = z
  .string()
  .min(1)
  .refine(
    (v) => v.trim().length > 0 && new TextEncoder().encode(v).length <= 80 && !/\p{Cc}/u.test(v),
    "Use a nonempty profile name of at most 80 UTF-8 bytes without control characters",
  );
const patchFields = <T extends z.ZodRawShape>(shape: T) =>
  z
    .object(shape)
    .strict()
    .refine((v) => Object.keys(v).some((k) => k !== "id"), "Choose a setting to change");
const empty = z.object({}).strict();
const rgbByte = z.number().int().min(0).max(255);
const lightingSentSchema = z
  .object({ color: z.tuple([rgbByte, rgbByte, rgbByte]), brightness: z.number().int().min(0).max(100) })
  .strict();
const lightingApplySchema = lightingSentSchema.extend({ id: identifier, allowHardware: z.literal(true) }).strict();
export type LightingApply = z.infer<typeof lightingApplySchema>;
const schemas: Record<string, z.ZodTypeAny> = {
  "state.get": empty,
  "streams.list": empty,
  "settings.get": empty,
  "profiles.list": empty,
  "devices.list": empty,
  "lighting.apply": lightingApplySchema,
  "stream.set": patchFields({
    id: identifier,
    volume: gain.optional(),
    muted: z.boolean().optional(),
    group: groupId.optional(),
    sinkId: numericId.optional(),
  }),
  "group.set": patchFields({
    id: groupId.exclude(["unmanaged"]),
    volume: gain.optional(),
    muted: z.boolean().optional(),
    wheelSide: side.optional(),
  }),
  "chatmix.set": patchFields({
    balance: z.number().finite().min(-1).max(1).optional(),
    enabled: z.boolean().optional(),
    inputMode: inputMode.optional(),
  }),
  "device.set": patchFields({
    id: identifier,
    hardwareEnabled: z.boolean().optional(),
    sidetone: z.number().int().min(0).max(3).optional(),
    autoOffMinutes: z.number().int().min(0).max(255).optional(),
    statusRefresh: z.boolean().optional(),
  }).refine(
    (v) =>
      !(
        v.hardwareEnabled === false &&
        (v.sidetone !== undefined || v.autoOffMinutes !== undefined || v.statusRefresh === true)
      ),
    "Release cannot be combined with hardware writes",
  ),
  "settings.set": z.object({ closeToTray: z.boolean() }).strict(),
  "profiles.save": z.object({ name: profileName }).strict(),
  "profiles.apply": z.object({ name: profileName }).strict(),
  // Availability is advertised by the backend; validation never implies physical verification.
};
export function validateCommand(method: string, params: unknown): unknown {
  const schema = Object.hasOwn(schemas, method) ? schemas[method] : undefined;
  if (!schema) throw new Error("Unsupported command");
  return schema.parse(params);
}
export const streamSchema = z.object({
  id: identifier,
  appKey: identifier,
  name: z.string().max(512),
  appName: z.string().max(256),
  volume: observedGain,
  effectiveVolume: observedGain.optional(),
  effectiveMuted: z.boolean().optional(),
  muted: z.boolean(),
  group: groupId,
  sinkId: numericId.nullable().optional(),
});
export const sinkSchema = z.object({
  id: numericId,
  name: z.string().max(512),
  description: z.string().max(512),
  volume: observedGain,
  muted: z.boolean(),
});
export const groupSchema = z.object({
  id: groupId.exclude(["unmanaged"]),
  name: z.string().max(64),
  volume: gain,
  muted: z.boolean(),
  wheelSide: side,
});
export const physicalSchema = z.object({
  deviceId: identifier.nullable().optional(),
  hardwareEnabled: z.boolean(),
  hardwareAcquired: z.boolean(),
  connected: z.boolean().nullable(),
  battery: z.number().min(0).max(100).nullable(),
  charging: z.boolean().nullable().optional(),
  sample: z
    .object({
      gamePercent: z.number().min(0).max(100),
      chatPercent: z.number().min(0).max(100),
      balance: z.number().min(-1).max(1),
      receivedAtMs: z.number().nonnegative(),
    })
    .nullable(),
  statusAtMs: z.number().nonnegative().nullable(),
  stale: z.boolean(),
  pending: z.boolean(),
  sidetone: z.number().int().min(0).max(3).nullable(),
  autoOffMinutesSent: z.number().int().min(0).max(255).nullable(),
  lastCommand: z.string().max(1024).nullable(),
  error: z.string().max(2048).nullable(),
});
export const deviceSchema = z.object({
  id: identifier,
  name: z.string().max(256),
  vendorId: z.number().int().min(0).max(65535),
  productId: z.number().int().min(0).max(65535),
  connected: z.boolean(),
  kind: z.enum(["headset", "keyboard", "other"]).default("other"),
  battery: z.number().min(0).max(100).nullable().optional(),
  physical: physicalSchema.optional(),
  lighting: z
    .object({
      lastSent: lightingSentSchema.nullable().optional(),
      pending: z.boolean().optional(),
      error: z.string().max(2048).nullable().optional(),
    })
    .optional(),
  capabilities: z
    .record(
      z.object({
        supported: z.boolean(),
        applicable: z.boolean().optional(),
        reason: z.string().max(1024).optional(),
        locallyValidated: z.boolean().optional(),
      }),
    )
    .default({}),
});
export const snapshotSchema = z.object({
  readOnly: z.boolean().optional(),
  streams: z.array(streamSchema).max(1024),
  sinks: z.array(sinkSchema).max(256).default([]),
  groups: z.array(groupSchema).max(3),
  devices: z.array(deviceSchema).max(128),
  chatmix: z.object({
    balance: z.number().min(-1).max(1),
    enabled: z.boolean(),
    inputMode: inputMode.optional(),
    wheelAvailable: z.boolean().default(false),
    reason: z.string().optional(),
  }),
  physical: physicalSchema.optional(),
  audio: z.object({ available: z.boolean(), reason: z.string().optional() }),
  profiles: z.array(z.object({ name: profileName })).default([]),
});
// Legacy/fixture snapshots have no sink inventory; the parsed bridge defaults it to [].
export type Snapshot = Omit<z.infer<typeof snapshotSchema>, "sinks"> & { sinks?: Sink[] };
export type Stream = z.infer<typeof streamSchema>;
export type Sink = z.infer<typeof sinkSchema>;
export type Group = z.infer<typeof groupSchema>;
export type Device = z.infer<typeof deviceSchema>;
export type StreamPatch = { id: string; volume?: number; muted?: boolean; group?: Stream["group"]; sinkId?: string };
export type GroupPatch = { id: Group["id"]; volume?: number; muted?: boolean; wheelSide?: Group["wheelSide"] };
export type RuntimeInfo = {
  readOnly?: boolean;
  connected: boolean;
  message: string;
  trayAvailable: boolean;
  closeToTray: boolean;
  transport: "sidecar" | "socket";
  testMode: boolean;
};
export interface DesktopBridge {
  getState(): Promise<Snapshot>;
  getRuntime(): Promise<RuntimeInfo>;
  setStream(value: StreamPatch): Promise<void>;
  setGroup(value: GroupPatch): Promise<void>;
  setChatmix(value: { balance?: number; enabled?: boolean; inputMode?: "software" | "hardware" }): Promise<void>;
  setDevice(value: {
    id: string;
    hardwareEnabled?: boolean;
    sidetone?: number;
    autoOffMinutes?: number;
    statusRefresh?: boolean;
  }): Promise<void>;
  applyLighting(value: LightingApply): Promise<void>;
  saveProfile(name: string): Promise<void>;
  applyProfile(name: string): Promise<void>;
  setCloseToTray(enabled: boolean): Promise<RuntimeInfo>;
  getArtwork(deviceId: string): Promise<string | null>;
  chooseArtwork(deviceId: string): Promise<string | null>;
  downloadArtwork(deviceId: string): Promise<string | null>;
}
declare global {
  interface Window {
    ssgg?: DesktopBridge;
  }
}
