#version 150

in vec2 v_uv;
in vec4 v_color;

uniform sampler2D u_texture;
uniform vec2 u_stepsize;
uniform bool u_horizontal;

out vec4 o_color;

void main() {
    vec2 direction = u_horizontal ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
    float sigma = 3.5;

    vec3 incremental_gauss;
    incremental_gauss.x = 1.0 / (sqrt(2.0 * 3.141592653589) * sigma);
    incremental_gauss.y = exp(-0.5 / (sigma * sigma));
    incremental_gauss.z = incremental_gauss.y * incremental_gauss.y;

    vec4 avg = vec4(0.0);
    float sum = 0.0;

    avg += texture(u_texture, v_uv) * incremental_gauss.x;
    sum += incremental_gauss.x;
    incremental_gauss.xy *= incremental_gauss.yz;

    for (float i = 1.0; i < 7.0; i++) {
        avg += texture(u_texture, v_uv.st - i * u_stepsize * direction) * incremental_gauss.x;
        avg += texture(u_texture, v_uv.st + i * u_stepsize * direction) * incremental_gauss.x;
        sum += 2.0 * incremental_gauss.x;
        incremental_gauss.xy *= incremental_gauss.yz;
    }

    o_color = v_color * avg / sum;
}
