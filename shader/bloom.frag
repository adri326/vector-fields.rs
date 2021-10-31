#version 150

in vec2 v_uv;
in vec4 v_color;

uniform sampler2D u_texture;
uniform float u_threshold;

out vec4 o_color;

void main() {
    vec4 color = v_color * texture(u_texture, v_uv);
    float luminance = dot(color.xyz, vec3(0.2125, 0.7154, 0.0721));
    luminance = max(0.0, luminance - u_threshold);
    color *= sign(luminance);
    color.a = 1.0;
    o_color = color;
}
