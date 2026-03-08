#!/usr/bin/env node
/**
 * Submit the E1M1 DoomMap to the on-chain pallet via sudo.
 * Usage: node set-map.mjs [ws://host:9944]
 */
import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';

const WS_URL = process.argv[2] || 'ws://159.89.88.85:9944';
const FP_SCALE = 1000;

// Helper: center of grid cell in fixed-point
function c(gx, gy) {
  return [gx * FP_SCALE + 500, gy * FP_SCALE + 500];
}

// ── Build the E1M1 map (mirrors engine/src/map.rs DoomMap::e1m1()) ──

const layout = [
  'WWWWWWWWWWWWWWWWWWWWWWW', // 0
  'W..........W..........W', // 1
  'W..........W..........W', // 2
  'W..........W..........W', // 3
  'W..........W..........W', // 4
  'WWWWW.WWWWWWWWWW.WWWWWW', // 5
  'W..........W..........W', // 6
  'W..........D..........W', // 7
  'W..........W..........W', // 8
  'WWWWWDWWWWWWWWWWWDWWWWW', // 9
  'W..........W..........W', // 10
  'W..........W..........W', // 11
  'W..........W..........W', // 12
  'W..........W..........W', // 13
  'WWWWWW.WWWWWWWWWWWWWWWW', // 14
  'W..........W..........W', // 15
  'W..........D..........W', // 16
  'W..........W..........W', // 17
  'WWWWWDWWWWWWWWWWW.WWWWW', // 18
  'W..........W..........W', // 19
  'W..........W..........W', // 20
  'W..........W..........W', // 21
  'W..........W.........XW', // 22
  'WWWWWWWWWWWWWWWWWWWWWWW', // 23
];

const sectorMap = [
  'WWWWWWWWWWWWWWWWWWWWWWW',
  'WSSSSSSSSSSWCCCCCCCCCCW',
  'WSSTTTTTTSSWCCCCCCCCCCW',
  'WSSTTTTTTSSWCCCCCCCCCCW',
  'WSSSSSSSSSSWCCCCCCCCCCW',
  'WWWWWSWWWWWWWWWWWCWWWWW',
  'WAAAAAAAAAAWCCCCCCCCCCW',
  'WAAAAAAAAAADCCCCCCCCCCW',
  'WAAAAAAAAAAWCCCCCCCCCCW',
  'WWWWWDWWWWWWWWWWWDWWWWW',
  'WHHHHHHHHHHWDDDDDDDDDDW',
  'WHHHHHHHHHHWDDDDDDDDDDW',
  'WHHHHHHHHHHWDDDDDDDDDDW',
  'WHHHHHHHHHHWDDDDDDDDDDW',
  'WWWWWWHWWWWWWWWWWWWWWWW',
  'WDDDDDDDDDDWKKKKKKKKKKW',
  'WDDDDDDDDDDWKKKKKKKKKKW',
  'WDDDDDDDDDDWKKKKKKKKKKW',
  'WWWWWDWWWWWWWWWWWKWWWWW',
  'WKKKKKKKKKKWEEEEEEEEEEW',
  'WKKKKKKKKKKWEEEEEEEEEEW',
  'WKKKKKKKKKKWEEEEEEEEEEW',
  'WKKKKKKKKKKWEEEEEEEEEEW',
  'WWWWWWWWWWWWWWWWWWWWWWW',
];

const width = layout[0].length;
const height = layout.length;

// Parse tiles
const tiles = [];
for (let row = 0; row < height; row++) {
  for (let col = 0; col < width; col++) {
    const ch = layout[row][col];
    switch (ch) {
      case 'W': tiles.push({ Wall: 0 }); break;
      case 'D': tiles.push({ Door: 'Closed' }); break;
      case 'X': tiles.push('Exit'); break;
      default:  tiles.push('Empty'); break;
    }
  }
}

// Parse sectors
function makeSector(ch) {
  switch (ch) {
    case 'S': return { floor_height: 0, ceiling_height: 1000, floor_tex: 3, ceiling_tex: 0, light_level: 180, light_effect: 'None' };
    case 'T': return { floor_height: 0, ceiling_height: 1000, floor_tex: 5, ceiling_tex: 0, light_level: 190, light_effect: 'None' };
    case 'A': return { floor_height: 0, ceiling_height: 1000, floor_tex: 5, ceiling_tex: 0, light_level: 140, light_effect: 'None' };
    case 'H': return { floor_height: 0, ceiling_height: 1100, floor_tex: 7, ceiling_tex: 255, light_level: 120, light_effect: 'Flicker' };
    case 'D': return { floor_height: 0, ceiling_height: 900,  floor_tex: 4, ceiling_tex: 1, light_level: 100, light_effect: 'Flicker' };
    case 'K': return { floor_height: 100, ceiling_height: 1000, floor_tex: 6, ceiling_tex: 0, light_level: 160, light_effect: 'None' };
    case 'E': return { floor_height: 200, ceiling_height: 900,  floor_tex: 7, ceiling_tex: 2, light_level: 60, light_effect: 'Pulse' };
    default:  return { floor_height: 0, ceiling_height: 1000, floor_tex: 3, ceiling_tex: 0, light_level: 160, light_effect: 'None' };
  }
}

const sectors = [];
for (let row = 0; row < height; row++) {
  for (let col = 0; col < width; col++) {
    const ch = sectorMap[row][col];
    sectors.push(makeSector(ch));
  }
}

const player_start = [2 * FP_SCALE + 500, 3 * FP_SCALE + 500, 0];

const enemy_spawns = [
  [c(5,7)[0],  c(5,7)[1],  'Imp'],
  [c(5,12)[0], c(5,12)[1], 'Imp'],
  [c(17,12)[0],c(17,12)[1],'Sergeant'],
  [c(5,16)[0], c(5,16)[1], 'Demon'],
  [c(5,20)[0], c(5,20)[1], 'Sergeant'],
  [c(17,21)[0],c(17,21)[1],'Imp'],
];

const item_spawns = [
  [c(5,1)[0],  c(5,1)[1],  'AmmoClip'],
  [c(19,1)[0], c(19,1)[1], 'HealthPack'],
  [c(5,8)[0],  c(5,8)[1],  'Shotgun'],
  [c(2,6)[0],  c(2,6)[1],  'ShellBox'],
  [c(9,6)[0],  c(9,6)[1],  'AmmoClip'],
  [c(5,12)[0], c(5,12)[1], 'Medikit'],
  [c(18,10)[0],c(18,10)[1],'Armor'],
  [c(3,15)[0], c(3,15)[1], 'AmmoClip'],
  [c(15,16)[0],c(15,16)[1],'ShellBox'],
  [c(5,19)[0], c(5,19)[1], 'HealthPack'],
  [c(9,20)[0], c(9,20)[1], 'AmmoBox'],
  [c(3,21)[0], c(3,21)[1], 'Chaingun'],
  [c(18,22)[0],c(18,22)[1],'Medikit'],
  [c(15,21)[0],c(15,21)[1],'RocketLauncher'],
  [c(19,20)[0],c(19,20)[1],'RocketBox'],
];

const decorations = [
  { x: c(4,4)[0],  y: c(4,4)[1],  deco_type: 'TallGreenTorch' },
  { x: c(6,4)[0],  y: c(6,4)[1],  deco_type: 'TallGreenTorch' },
  { x: c(5,2)[0],  y: c(5,2)[1],  deco_type: 'DeadPlayer' },
  { x: c(20,2)[0], y: c(20,2)[1], deco_type: 'Barrel' },
  { x: c(1,6)[0],  y: c(1,6)[1],  deco_type: 'Barrel' },
  { x: c(1,8)[0],  y: c(1,8)[1],  deco_type: 'Barrel' },
  { x: c(10,8)[0], y: c(10,8)[1], deco_type: 'Barrel' },
  { x: c(6,12)[0], y: c(6,12)[1], deco_type: 'Candelabra' },
  { x: c(1,10)[0], y: c(1,10)[1], deco_type: 'TallRedTorch' },
  { x: c(10,13)[0],y: c(10,13)[1],deco_type: 'TallRedTorch' },
  { x: c(5,10)[0], y: c(5,10)[1], deco_type: 'SkullsAndCandles' },
  { x: c(20,11)[0],y: c(20,11)[1],deco_type: 'SkullOnStick' },
  { x: c(17,12)[0],y: c(17,12)[1],deco_type: 'DeadPlayer' },
  { x: c(5,15)[0], y: c(5,15)[1], deco_type: 'HangingBody' },
  { x: c(1,19)[0], y: c(1,19)[1], deco_type: 'Column' },
  { x: c(10,19)[0],y: c(10,19)[1],deco_type: 'Column' },
  { x: c(14,20)[0],y: c(14,20)[1],deco_type: 'EvilEye' },
  { x: c(14,22)[0],y: c(14,22)[1],deco_type: 'TallRedPillar' },
  { x: c(20,22)[0],y: c(20,22)[1],deco_type: 'TallRedPillar' },
];

const doomMap = {
  width,
  height,
  tiles,
  sectors,
  player_start,
  enemy_spawns,
  item_spawns,
  decorations,
};

// ── Submit via sudo ──

async function main() {
  console.log(`Connecting to ${WS_URL}...`);
  const provider = new WsProvider(WS_URL);
  const api = await ApiPromise.create({ provider });
  console.log(`Connected. Chain: ${(await api.rpc.system.chain()).toString()}`);

  // Alice is sudo in --dev mode
  const keyring = new Keyring({ type: 'sr25519' });
  const alice = keyring.addFromUri('//Alice');
  console.log(`Sudo account: ${alice.address}`);

  // Construct the setMap call
  const setMapCall = api.tx.doom.setMap(doomMap);
  console.log('Constructed doom.setMap call');

  // Wrap in sudo
  const sudoCall = api.tx.sudo.sudo(setMapCall);
  console.log('Submitting sudo.sudo(doom.setMap(...))...');

  const unsub = await sudoCall.signAndSend(alice, ({ status, events, dispatchError }) => {
    if (status.isInBlock) {
      console.log(`Included in block: ${status.asInBlock.toHex()}`);

      // Check for errors
      if (dispatchError) {
        if (dispatchError.isModule) {
          const decoded = api.registry.findMetaError(dispatchError.asModule);
          console.error(`ERROR: ${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`);
        } else {
          console.error(`ERROR: ${dispatchError.toString()}`);
        }
      }

      // Check sudo result
      for (const { event } of events) {
        if (event.section === 'sudo' && event.method === 'Sudid') {
          const result = event.data[0];
          if (result.isOk) {
            console.log('SUCCESS: Map set on-chain!');
          } else {
            console.error('SUDO FAILED:', result.asErr.toString());
          }
        }
      }

      unsub();
      process.exit(0);
    }
  });
}

main().catch((err) => {
  console.error('Fatal:', err);
  process.exit(1);
});
