// The WebGL 2 renderer. It uploads what the core returns and draws it, in
// four passes: the solid world and the other surfers, then see-through
// things (netting and ghosts), then your hands over everything. Every size,
// stride and offset comes from the core's numbers(); this file holds no
// layout of its own. After Raiders of the Lost Mart's web/gl.js.

function compile(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader));
  return shader;
}

function link(gl, vertexSource, fragmentSource) {
  const program = gl.createProgram();
  gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, vertexSource));
  gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, fragmentSource));
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program));
  return program;
}

// Returns null when the browser has no WebGL 2; the page then says so.
export function makeRenderer(canvas, N, shaders, display) {
  const gl = canvas.getContext("webgl2", { antialias: true, powerPreference: "high-performance" });
  if (!gl) return null;
  const program = link(gl, shaders.vertex, shaders.fragment);
  gl.useProgram(program);
  gl.enable(gl.DEPTH_TEST);
  gl.enable(gl.CULL_FACE);
  gl.bindVertexArray(gl.createVertexArray());

  const attributes = N.attributes.map(([name, size, offset]) => ({ loc: gl.getAttribLocation(program, name), size, offset }));
  for (const a of attributes) gl.enableVertexAttribArray(a.loc);
  const uVP = gl.getUniformLocation(program, "uVP");
  const uEye = gl.getUniformLocation(program, "uEye");
  const uTime = gl.getUniformLocation(program, "uTime");
  const uPass = gl.getUniformLocation(program, "uPass");
  const [hr, hg, hb] = display.haze;
  gl.clearColor(hr, hg, hb, 1);

  const world = gl.createBuffer();
  const others = gl.createBuffer();
  const hands = gl.createBuffer();
  let worldCounts = [0, 0];
  let othersCounts = [0, 0];
  let handsCount = 0;

  function bind(buffer) {
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    for (const a of attributes) gl.vertexAttribPointer(a.loc, a.size, gl.FLOAT, false, N.stride, a.offset);
  }

  function solid() {
    gl.disable(gl.BLEND);
    gl.depthMask(true);
    gl.enable(gl.CULL_FACE);
  }

  function seeThrough() {
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.depthMask(false);
    gl.disable(gl.CULL_FACE);
  }

  return {
    setWorld(vertices, counts) {
      gl.bindBuffer(gl.ARRAY_BUFFER, world);
      gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
      worldCounts = counts;
    },
    resize(size) {
      const [w, h] = size;
      canvas.width = w;
      canvas.height = h;
      gl.viewport(0, 0, w, h);
    },
    frame(view, othersVertices, othersOpaque, handsVertices) {
      gl.bindBuffer(gl.ARRAY_BUFFER, others);
      gl.bufferData(gl.ARRAY_BUFFER, othersVertices, gl.STREAM_DRAW);
      othersCounts = [othersOpaque, othersVertices.length / N.floats_per_vertex];
      gl.bindBuffer(gl.ARRAY_BUFFER, hands);
      gl.bufferData(gl.ARRAY_BUFFER, handsVertices, gl.STREAM_DRAW);
      handsCount = handsVertices.length / N.floats_per_vertex;

      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
      gl.uniformMatrix4fv(uVP, false, view.subarray(...N.view.matrix));
      gl.uniform3fv(uEye, view.subarray(...N.view.eye));
      gl.uniform1f(uTime, view[N.view.time]);

      solid();
      gl.uniform1f(uPass, 0);
      bind(world);
      gl.drawArrays(gl.TRIANGLES, 0, worldCounts[0]);
      bind(others);
      gl.drawArrays(gl.TRIANGLES, 0, othersCounts[0]);

      seeThrough();
      gl.uniform1f(uPass, 1);
      bind(others);
      gl.drawArrays(gl.TRIANGLES, othersCounts[0], othersCounts[1] - othersCounts[0]);
      bind(world);
      gl.drawArrays(gl.TRIANGLES, worldCounts[0], worldCounts[1] - worldCounts[0]);

      // the hands are never behind a wall
      solid();
      gl.clear(gl.DEPTH_BUFFER_BIT);
      gl.uniform1f(uPass, display.hands_pass);
      bind(hands);
      gl.drawArrays(gl.TRIANGLES, 0, handsCount);
    },
  };
}
