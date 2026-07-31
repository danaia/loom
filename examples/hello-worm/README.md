# Hello Worm

`hello_worm_builder()` creates an interactive Pqo species called the **Scent
Weaver**. Its 24 linked body segments, food state, smell signal, steering memory,
meal count, and camera are persistent GPU streams.

Run it on macOS with Metal:

```text
./scripts/run-hello-particle.sh worm
```

Controls:

- Click the plane to drop a food pellet.
- Left-drag to orbit the camera.
- Scroll to zoom.

The worm searches active food slots, chooses the nearest scent, turns toward it
with persistent heading memory, avoids the plane edge, articulates its body, and
consumes food when its head reaches the pellet. A brighter head means a stronger
smell signal.
