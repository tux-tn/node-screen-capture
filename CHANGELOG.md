## [1.1.0](https://github.com/tux-tn/node-screen-capture/compare/v1.0.0...v1.1.0) (2026-07-17)

### Features

* add macOS capture and harden platform backends ([9c5254e](https://github.com/tux-tn/node-screen-capture/commit/9c5254e43833ce877143a224198bdc68ad984a67))

### Bug Fixes

* **macos:** pin apple-metal for macOS 15 SDK ([a1e0e30](https://github.com/tux-tn/node-screen-capture/commit/a1e0e30718954333abd2d8090b7dfbb685d9135c))
* **macos:** show macOS content picker ([ecb8782](https://github.com/tux-tn/node-screen-capture/commit/ecb87820fb6b14eb2008872dbee616a215ff5999))
* **macos:** stabilize capture frame delivery ([fc06cbe](https://github.com/tux-tn/node-screen-capture/commit/fc06cbe9dc26073e01aac79014b09e11ef88b007))
* **native:** harden backend resource handling ([e912ee3](https://github.com/tux-tn/node-screen-capture/commit/e912ee3b179100b71f69fb58321eaf632e9364c1))
* silence unused Windows message result ([fa06df0](https://github.com/tux-tn/node-screen-capture/commit/fa06df07992f652e0b804b1ae8906ed019bb4159))

## 1.0.0 (2026-07-14)

### Features

* added TypeScript source layer with async frame queue ([e8e0274](https://github.com/tux-tn/node-screen-capture/commit/e8e0274767cf805181f19e6a03abe18da95aa7fd))
* **ci:** add Linux build and test matrix ([93fea43](https://github.com/tux-tn/node-screen-capture/commit/93fea437205d608270aef74731f9579f1d6e20ed))
* implemented Windows DXGI desktop duplication capture ([2c31834](https://github.com/tux-tn/node-screen-capture/commit/2c31834a7991e12dce02cf6a9c745f4fc18ce7b1))
* **linux:** add Wayland/PipeWire screen capture backend ([55ccbc4](https://github.com/tux-tn/node-screen-capture/commit/55ccbc45ee5b0044b32f26cabdf1d47edc895396))
* scaffolded windows-capture napi-rs native addon project ([b4e6ace](https://github.com/tux-tn/node-screen-capture/commit/b4e6acea6f89a7789c60dfba379d17d05568d331))
* **smoke:** save captured frames to tmp directory ([f0e1e41](https://github.com/tux-tn/node-screen-capture/commit/f0e1e41bfcbba6dff0f5baf4c66d79a21f360ee7))
* **testing:** add unit tests with mocked native addon ([29e253c](https://github.com/tux-tn/node-screen-capture/commit/29e253c89e062cdf6b3b9387109065fe801b3396))
