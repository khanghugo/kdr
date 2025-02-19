# kdr - Khang's Demo Renderer

Currently work in progress, check back later

## Progresses - Subjected to changes and addition but not subtraction.

- Renderer
  - BSP
    - [X] Face
    - [X] Texture
    - [X] Lightmap
    - [ ] Weird math lightmap in the case of older compiled HL maps
    - [ ] Light styles (unplanned) 
    - [X] Transparency 
    - [ ] Named entities. Some entities aren't properly displaced
    - [X] Samey shader as the game
  - MDL
    - [ ] Face
    - [X] Texture
  - [ ] Skybox
  - Optimization
    - [X] Lightmap atlas
    - [X] Batch rendering based on texture
    - [ ] Array of texture
    - [ ] Transparency sorting
    - [ ] Visibility. At the moment it renders everything
- Navigation
  - [X] Noclip movement
  - [X] Pitch and Yaw
  - [ ] GoldSrc movement (unplanned)
  - [ ] Mouse view
- Demo Player
  - [ ] Demo. Easy to do because this is the same code I have in other two projects
  - [ ] Ghost. Same thing. Very easy to implement
- Demo Renderer
  - [ ] Framebuffer
  - [ ] Remux
- [X] BSP viewer. It is implicitly one.
- Integration
  - [X] Native with Vulkan
  - [ ] Web with WebGPU. Probably working just fine. Just need to write some HTML
  - [ ] Formats handling. Loads .bsp or .dem and displays them
  - [ ] User interface. `egui` probably works
