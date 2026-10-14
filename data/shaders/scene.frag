#version 300 es
// The gym's look: each material number (crates/core/src/mesh.rs) is a
// pattern painted from the world position. Patterns only; no rule of the game.
precision highp float;
in vec3 vPos;
in vec3 vNormal;
in vec3 vColor;
flat in int vMat;
uniform vec3 uEye;
uniform float uTime;
uniform float uPass; // 0 solid, 1 see-through, 2 hands
out vec4 outColor;

const vec3 HAZE = vec3(0.86, 0.88, 0.90);
const vec3 SUN = normalize(vec3(0.35, 0.22, 0.91));

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
// a thin line at every whole x, faded to its average shade once it is finer
// than a pixel (so distant walls do not shimmer)
float line(float x, float w) {
  float fw = fwidth(x);
  float f = abs(fract(x) - 0.5);
  float l = smoothstep(0.5 - w - fw, 0.5 - w + fw, f);
  return mix(l, 2.0 * w, clamp(fw * 1.5, 0.0, 1.0));
}

// two axes in the surface: across (horizontal) and up the surface
void surfaceAxes(vec3 n, out vec3 t, out vec3 b) {
  vec3 h = abs(n.z) > 0.95 ? vec3(1.0, 0.0, 0.0) : normalize(vec3(-n.y, n.x, 0.0));
  t = h;
  b = normalize(cross(n, t));
}

vec3 paint(int m, vec3 base, vec3 p, vec3 n, out float alpha, out float glow) {
  alpha = 1.0;
  glow = 0.0;
  vec3 t, b;
  surfaceAxes(n, t, b);
  float u = dot(p, t), v = dot(p, b);
  if (m == 1) {
    // gym mat vinyl: stitched panels across the ramp, piping along it
    float seam = line(u / 128.0, 0.012);
    float pipe = line(v / 512.0, 0.006);
    vec3 c = base * (0.94 + 0.06 * hash(floor(vec2(u / 128.0, 0.0))));
    c = mix(c, base * 0.55, max(seam, pipe * 0.7));
    return c;
  }
  if (m == 2) {
    // maple boards
    float board = floor(p.x / 24.0);
    float run = floor((p.y + hash(vec2(board, 1.0)) * 300.0) / 300.0);
    vec3 c = base * (0.9 + 0.16 * hash(vec2(board, run)));
    c = mix(c, base * 0.7, line(p.x / 24.0, 0.02) * 0.6);
    return c;
  }
  if (m == 3) {
    // painted cinderblock, with a stripe band like a school gym's
    float row = floor(p.z / 32.0);
    float bu = u / 64.0 + (mod(row, 2.0) * 0.5);
    float mortar = max(line(p.z / 32.0, 0.05), line(bu, 0.03));
    vec3 c = mix(base, base * 0.78, mortar);
    float band = mod(p.z, 1024.0);
    if (band > 380.0 && band < 470.0) c = mix(c, vec3(0.18, 0.36, 0.75), 0.85);
    if (band > 470.0 && band < 500.0) c = mix(c, vec3(0.98, 0.78, 0.22), 0.85);
    return c;
  }
  if (m == 4) {
    // ceiling tiles
    float g = max(line(p.x / 64.0, 0.02), line(p.y / 64.0, 0.02));
    return mix(base, base * 0.75, g);
  }
  if (m == 5) {
    // safety netting
    float g = max(line(u / 24.0, 0.05), line(v / 24.0, 0.05));
    alpha = 0.10 + 0.55 * g;
    return mix(base, vec3(0.3, 0.35, 0.45), g);
  }
  if (m == 6) {
    // a pit of foam cubes
    vec2 cell = floor(p.xy / 40.0);
    vec2 f = fract(p.xy / 40.0);
    float k = hash(cell);
    vec3 c = k < 0.2 ? vec3(0.95, 0.45, 0.55) : k < 0.4 ? vec3(0.35, 0.60, 0.95) : k < 0.6 ? vec3(0.98, 0.82, 0.28) : k < 0.8 ? vec3(0.45, 0.82, 0.50) : vec3(0.98, 0.60, 0.25);
    float edge = min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y));
    c *= 0.65 + 0.35 * smoothstep(0.0, 0.18, edge);
    return c;
  }
  if (m == 7) { glow = 1.0; return base; }
  if (m == 8) {
    // the gymnasium around everything
    if (n.z > 0.5) {
      vec3 c = vec3(0.80, 0.60, 0.38) * (0.9 + 0.1 * hash(floor(p.xy / vec2(48.0, 400.0))));
      float court = line(length(p.xy) / 3000.0, 0.004) + line(p.x / 6000.0, 0.002);
      return mix(c, vec3(0.95, 0.95, 0.95), clamp(court, 0.0, 1.0));
    }
    if (n.z < -0.5) {
      float truss = max(line(p.x / 1024.0, 0.01), line(p.y / 1024.0, 0.01));
      vec2 lc = fract(p.xy / 2048.0);
      float lamp = step(0.45, lc.x) * step(lc.x, 0.55) * step(0.45, lc.y) * step(lc.y, 0.55);
      glow = lamp;
      return mix(mix(vec3(0.70, 0.72, 0.76), vec3(0.35, 0.38, 0.45), truss), vec3(1.0, 0.98, 0.9), lamp);
    }
    float row = floor(p.z / 48.0);
    float bu = u / 96.0 + mod(row, 2.0) * 0.5;
    vec3 c = mix(vec3(0.90, 0.88, 0.82), vec3(0.72, 0.70, 0.66), max(line(p.z / 48.0, 0.04), line(bu, 0.02)));
    // high windows, and pennants hung between them
    float wz = p.z - 9000.0;
    float wu = fract(u / 3000.0);
    if (wz > 0.0 && wz < 3000.0 && wu > 0.1 && wu < 0.9) { glow = 0.6; c = vec3(0.80, 0.90, 1.0); }
    float pu = fract(u / 1500.0) * 2.0 - 1.0;
    float pz = (p.z - 5200.0) / 1600.0;
    if (pz > 0.0 && pz < 1.0 && abs(pu) < 0.3 * (1.0 - pz)) {
      float k = hash(vec2(floor(u / 1500.0), 3.0));
      c = k < 0.33 ? vec3(0.85, 0.20, 0.22) : k < 0.66 ? vec3(0.18, 0.40, 0.85) : vec3(0.98, 0.78, 0.20);
    }
    return c;
  }
  if (m == 9) {
    float ch = mod(floor(u / 24.0) + floor(v / 24.0), 2.0);
    glow = 0.5 + 0.3 * sin(uTime * 4.0);
    return mix(base, vec3(0.1, 0.1, 0.12), ch * 0.8);
  }
  if (m == 10 || m == 15) {
    glow = 0.35 + 0.25 * sin(uTime * 3.0 + p.z * 0.02);
    return mix(base, base * 0.6, line(v / 32.0, 0.1));
  }
  if (m == 11) {
    vec2 cell = floor(p.xy / 12.0);
    float d = length(fract(p.xy / 12.0) - 0.5);
    float k = hash(cell);
    vec3 c = k < 0.25 ? vec3(0.95, 0.3, 0.3) : k < 0.5 ? vec3(0.3, 0.55, 0.95) : k < 0.75 ? vec3(0.98, 0.85, 0.3) : vec3(0.4, 0.85, 0.45);
    return mix(c, c * 0.5, smoothstep(0.38, 0.5, d));
  }
  if (m == 16) {
    // a kitsune platform: a lit grid in its stage's colour, on dark
    float g = max(line(p.x / 32.0, 0.04), line(p.y / 32.0, 0.04));
    glow = 0.35 + 0.65 * g;
    return mix(base * 0.28, base, g);
  }
  if (m == 13) {
    alpha = 0.38;
    return mix(base, vec3(0.75, 0.90, 1.0), 0.35);
  }
  return base;
}

void main() {
  vec3 n = normalize(vNormal);
  float alpha, glow;
  vec3 c = paint(vMat, vColor, vPos, n, alpha, glow);
  float diffuse = max(dot(n, SUN), 0.0);
  float sky = 0.5 + 0.5 * n.z;
  float light = 0.42 + 0.38 * diffuse + 0.22 * sky;
  vec3 lit = mix(c * light, c, glow);
  if (vMat == 1) {
    // vinyl has a soft sheen
    vec3 view = normalize(uEye - vPos);
    float spec = pow(max(dot(reflect(-SUN, n), view), 0.0), 24.0);
    lit += vec3(0.18) * spec;
  }
  if (uPass > 1.5) {
    outColor = vec4(lit, 1.0);
    return;
  }
  float d = length(vPos - uEye);
  float fog = clamp((d - 2500.0) / 42000.0, 0.0, 0.85);
  outColor = vec4(mix(lit, HAZE, fog), alpha);
}
