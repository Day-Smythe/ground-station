gui-ground-station/
├── Makefile                 # Build automation script
├── README.md                # Project documentation
├── include/                 # All header files (.h)
│   ├── ascii_map.h          # Math for 3D sphere projection & ASCII dithering
│   ├── radio_interface.h    # Hardware abstraction (SDR/Serial/UDP)
│   ├── triangulation.h      # TDoA or AoA intersection algorithms
│   └── ui.h                 # Core engine, window management, and input loops
└── src/                     # All source code files (.c)
    ├── main.c               # Application entry point (GLFW init)
    ├── ascii_map.c          # Glyph-rendering logic, 3D rotations, topography data
    ├── radio_interface.c    # Mock or real radio signal capturing 
    ├── triangulation.c      # Calculates coordinates from multiple signal lines
    └── ui.c                 # Handles my window updates and text rendering
