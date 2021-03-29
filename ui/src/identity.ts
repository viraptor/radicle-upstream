import * as remote from "./remote";
import * as proxy from "./proxy";
import type { Identity } from "./proxy/identity";

export type { Identity };

// TYPES
// FIXME(xla): Improve type safety of it, this is a placeholder to avoid using strings everywhere.
export type PeerId = string;

// STATE
const creationStore = remote.createStore<Identity>();
export const store = creationStore.readable;

export const createIdentity = (
  params: proxy.IdentityCreateParams
): Promise<Identity> => {
  return proxy.client.identityCreate(params);
};

export const fetch = (urn: string): Promise<Identity> => {
  return proxy.client.identityGet(urn);
};

// MOCK
export const fallback: Identity = {
  avatarFallback: {
    background: {
      r: 122,
      g: 112,
      b: 90,
    },
    emoji: "💡",
  },
  metadata: {
    handle: "cloudhead",
  },
  peerId: "hwd1yreyza9z77xzp1rwyxw9uk4kdrrzag5uybd7w1ihke18xxhxn6qu4oy",
  shareableEntityIdentifier:
    "rad:git:hwd1yreyza9z77xzp1rwyxw9uk4kdrrzag5uybd7w1ihke18xxhxn6qu4oy",
  urn: "rad:git:hwd1yreyza9z77xzp1rwyxw9uk4kdrrzag5uybd7w1ihke18xxhxn6qu4oy",
};
