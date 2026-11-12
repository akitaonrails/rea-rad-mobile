$input v_texcoord0

// license:MIT OR Apache-2.0
// Rad Mobile launcher: rea-crt chain, pass 4 (CRT composite).
// After Timothy Lottes' public-domain CRT shader; parameters after
// Wanszai's Scud Race PC port.
//
// Barrel warp, luminance-dependent Gaussian scanline beam, a 3-pixel
// phosphor mask in output-pixel space (aperture grille or shadow mask),
// additive glow from the blurred bloom, vignette and brightness.

#include "common.sh"

SAMPLER2D(s_screen, 0);
SAMPLER2D(s_glow, 1);

uniform vec4 u_tex_size0;   // game screen texture (width, height)
uniform vec4 u_target_dims; // this pass's output, in pixels
uniform vec4 u_scan;        // x = scanline strength, y = beam min, z = beam max, w = virtual lines (0: game screen height)
uniform vec4 u_mask;        // x = mask strength, y = type (0 aperture grille, 1 shadow mask), z = glow strength, w = brightness
uniform vec4 u_geom;        // x = curvature, y = vignette

vec2 barrel(vec2 uv, float curvature)
{
	uv = uv * 2.0 - 1.0;
	vec2 o = abs(uv.yx) / vec2(3.0, 4.0);
	uv += uv * o * o * curvature;
	return uv * 0.5 + 0.5;
}

vec3 phosphor(vec2 px, float shadow)
{
	// every other row moves by half a triad in the shadow-mask layout
	float shift = shadow * mod(floor(px.y), 2.0) * 1.5;
	float slot = mod(floor(px.x) + shift, 3.0);
	if (slot < 1.0) return vec3(1.0, 0.0, 0.0);
	if (slot < 2.0) return vec3(0.0, 1.0, 0.0);
	return vec3(0.0, 0.0, 1.0);
}

void main()
{
	vec2 uv = v_texcoord0;
	vec2 wuv = barrel(uv, u_geom.x);
	if (wuv.x < 0.0 || wuv.x > 1.0 || wuv.y < 0.0 || wuv.y > 1.0)
	{
		gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
		return;
	}

	vec3 src = texture2D(s_screen, wuv).rgb;
	vec3 glow = texture2D(s_glow, wuv).rgb;

	// scanline beam: dist in units of one line, width grows with luminance
	float lines = u_scan.w > 0.0 ? u_scan.w : u_tex_size0.y;
	float luma = dot(src, vec3(0.299, 0.587, 0.114));
	float dist = fract(wuv.y * lines) - 0.5;
	float w = mix(u_scan.y, u_scan.z, luma);
	float x = dist / w;
	float b = exp(-2.0 * x * x);
	vec3 col = src * mix(1.0 - u_scan.x, 1.0, b);

	vec3 mask = phosphor(v_texcoord0 * u_target_dims.xy, u_mask.y);
	col *= mix(vec3_splat(1.0), mask, vec3_splat(u_mask.x));

	col += glow * u_mask.z;

	vec2 vd = uv * 2.0 - 1.0;
	col *= saturate(1.0 - dot(vd, vd) * u_geom.y);
	col *= u_mask.w;

	gl_FragColor = vec4(saturate(col), 1.0);
}
