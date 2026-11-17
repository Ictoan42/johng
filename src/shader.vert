#version 140

in vec3 pos;
in vec2 tex_coords;
out vec2 v_tex_coords;

uniform float aspect_ratio;
uniform vec2 offset;
uniform vec2 scale;

void main() {
  v_tex_coords = tex_coords;
  vec3 position = pos;
  position.x *= scale.x;
  position.y *= scale.y;
  position.x /= aspect_ratio;
  position.x += offset.x;
  position.y += offset.y;
  gl_Position = vec4(position, 1.0);
}
