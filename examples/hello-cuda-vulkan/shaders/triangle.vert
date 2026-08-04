#version 460

layout(location = 0) out vec3 color;

void main() {
    const vec2 positions[3] = vec2[3](
        vec2(0.0, -0.6),
        vec2(0.6, 0.6),
        vec2(-0.6, 0.6)
    );
    const vec3 colors[3] = vec3[3](
        vec3(0.1, 0.8, 1.0),
        vec3(0.9, 0.2, 0.8),
        vec3(0.4, 1.0, 0.2)
    );
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    color = colors[gl_VertexIndex];
}
