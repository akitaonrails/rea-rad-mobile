$input a_position, a_texcoord0, a_color0
$output v_texcoord0

// license:MIT OR Apache-2.0
// Rad Mobile launcher: rea-crt chain, shared full-screen-quad vertex shader.

#include "common.sh"

uniform vec4 u_inv_view_dims;

void main()
{
	gl_Position = mul(u_viewProj, vec4(a_position.xy, 0.0, 1.0));
#if BGFX_SHADER_LANGUAGE_HLSL && BGFX_SHADER_LANGUAGE_HLSL <= 300
	gl_Position.xy += u_inv_view_dims.xy * gl_Position.w;
#endif
	v_texcoord0 = a_texcoord0;
}
