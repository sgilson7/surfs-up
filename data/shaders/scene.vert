#version 300 es
// Every vertex comes from the core already in world space (the hands too:
// the core places them in front of the eye). This shader only projects.
in vec3 aPos;
in vec3 aNormal;
in vec3 aColor;
in float aMat;
uniform mat4 uVP;
out vec3 vPos;
out vec3 vNormal;
out vec3 vColor;
flat out int vMat;
void main() {
  vPos = aPos;
  vNormal = aNormal;
  vColor = aColor;
  vMat = int(aMat + 0.5);
  gl_Position = uVP * vec4(aPos, 1.0);
}
