import Big from "big.js";
import * as ethers from "ethers";
import persistentStore from "svelte-persistent-store/dist";
import * as svelteStore from "svelte/store";
import type { Identity } from "./identity";
import * as session from "./session";

import type { UnsealedSession } from "./session";

import type { Wallet } from "./wallet";

// The Ethereum environments we support and may connect to.
export enum Environment {
  // A local node for testing and development. The test wallet we use
  // for this environment may be in the 'Ropsten', 'Mainnet', or other
  // network. However, those would not be the real 'Ropsten' and 'Mainnet'
  // networks, respectively, but simply fake ones for testing purposes.
  Local = "Local",
  // The Ropsten testnet network
  Ropsten = "Ropsten",
  // N.B: We will support 'Mainnet' in the future
}

// The ethereum networks we may parse from a connected wallet across
// all `Environment`s. See `supportedNetwork` to learn which networks
// each Environment supports.
export enum Network {
  Ropsten = "Ropsten",
  Mainnet = "Mainnet",
  Other = "Other",
}

// Inform which `Network`s we support within the given environment.
export function supportedNetwork(environment: Environment): Network {
  switch (environment) {
    case Environment.Local:
      return Network.Mainnet;
    case Environment.Ropsten:
      return Network.Ropsten;
  }
}

// Parse a `Network` value given a `chainId`.
// For reference check https://chainid.network.
export function networkFromChainId(chainId: number): Network {
  switch (chainId) {
    case 1:
      return Network.Mainnet;
    case 3:
      return Network.Ropsten;
    default:
      return Network.Other;
  }
}

// The store where the selected Ethereum environment is persisted.
export const selectedEnvironment = persistentStore.local.writable<Environment>(
  "ethereum-environment-v0",
  Environment.Ropsten
);

// EIP-20 token decimals for the tokens we operate with across
// the diferent environments. We hardcode this value since it
// is well-settled and since we would need to request it from
// the token contract for each number conversion otherwise.
// We have, however, to keep in mind that new versions of the
// token might change it.
const TOKEN_DECIMALS = Big(10).pow(18);

// Big.PE determines the exponent at which its `toString()` representation
// starts being displayed in exponential notation. We never want to do that.
Big.PE = Number.MAX_SAFE_INTEGER;

export function toBaseUnit(n: ethers.BigNumber | Big): Big {
  return Big(n.toString()).div(TOKEN_DECIMALS).round(2);
}

export function fromBaseUnit(n: Big): ethers.BigNumber {
  return ethers.BigNumber.from(n.mul(TOKEN_DECIMALS).round().toString());
}

export enum AttestationStatus {
  Fetching = "Fetching",
  Incomplete = "Incomplete",
  Expired = "Expired",
  Unmatching = "Unmatching",
  Valid = "Valid",
}

export const attestationStatus: svelteStore.Writable<AttestationStatus> = svelteStore.writable(
  AttestationStatus.Fetching
);

let watching = false;

function updateAttesttationStatus(walletStore: svelteStore.Readable<Wallet>) {
  const wallet = svelteStore.get(walletStore);
  const sess: UnsealedSession | undefined = session.unsealed();
  const ethAccount = wallet.account();
  if (sess && ethAccount) {
    attestationStatus.set(
      getAttestationStatus(sess.identity, ethAccount.address)
    );
  } else {
    attestationStatus.set(AttestationStatus.Fetching);
  }
}

export async function watchAttestationStatus(
  walletStore: svelteStore.Readable<Wallet>
): Promise<void> {
  if (!watching) {
    updateAttesttationStatus(walletStore);

    // FIXME: we don't want to poll, but to re-check the status based on
    // changes to the wallet(new account selected, new environment, etc).
    // We can't have updateAttesttationStatus derive wallet.store because of
    // initialization issues tho.
    const POLL_INTERVAL_MILLIS = 1000;
    setInterval(() => {
      updateAttesttationStatus(walletStore);
    }, POLL_INTERVAL_MILLIS);

    watching = true;
  }
}

// TODO(funding): we also need to lookup whether the identity.metadata.ethereum?.address has claimed
// the identity on chain using the ClaimsContract.claimed method.
function getAttestationStatus(
  identity: Identity,
  ethAddress: string
): AttestationStatus {
  const optEthClaim = identity.metadata.ethereum;
  if (optEthClaim === null) {
    return AttestationStatus.Incomplete;
  } else if (optEthClaim.expiration < new Date()) {
    return AttestationStatus.Expired;
  } else {
    return optEthClaim.address === ethAddress
      ? AttestationStatus.Valid
      : AttestationStatus.Unmatching;
  }
}
