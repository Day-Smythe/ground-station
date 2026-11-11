/* ===========================================================================
 *  radio_mapper.c
 *
 *  A deliberately boring OpenGL 2.1 / fixed-function C application.
 *  No shaders, no VBOs, no FBOs, no NPOT textures, no depth buffer.
 *  Everything is drawn with immediate mode (glBegin / glEnd) so the code
 *  stays easy to read and works on an ancient laptop.
 *
 *  Target: 1280 x 800 window.
 *
 *      [ LEFT 300 px ] [ CENTER 680 px ] [ RIGHT 300 px ]
 *        waypoints          colour vector      candidate
 *        + sliders          field / map        source spots
 *
 *  Build (Linux):
 *      cc -std=c99 -O2 radio_mapper.c -o radio_mapper -lglfw -lGL -lm
 *
 *  Build (macOS, Homebrew):
 *      cc -std=c99 -O2 radio_mapper.c -o radio_mapper \
 *         -I/opt/homebrew/include -L/opt/homebrew/lib \
 *         -lglfw -framework OpenGL -lm
 * ===========================================================================
 */

#include <GLFW/glfw3.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>

/* ---------------------------------------------------------------------------
 *  Constants
 * ------------------------------------------------------------------------- */
#define WIN_W 1280
#define WIN_H 800
#define PI_F  3.14159265358979f

#define LEFT_PANEL_X    0.0f
#define LEFT_PANEL_W    300.0f
#define RIGHT_PANEL_W   300.0f
#define RIGHT_PANEL_X   ((float)WIN_W - RIGHT_PANEL_W)          /* 980 */
#define CENTER_PANEL_X  (LEFT_PANEL_W)                          /* 300 */
#define CENTER_PANEL_W  (RIGHT_PANEL_X - LEFT_PANEL_W)          /* 680 */

#define WORLD_SIZE      1000.0f   /* metres across the map */
#define NUM_WAYPOINTS   6
#define MAX_CANDIDATES  5

#define VEC_TEX_SIZE    128

/* ---------------------------------------------------------------------------
 *  5 x 7 bitmap font, ASCII 0x20 .. 0x7E.
 *  One byte per column; bit 0 is the top row. Same data used for every
 *  label in the UI so we don't depend on any font library.
 * ------------------------------------------------------------------------- */
static const unsigned char FONT5X7[95 * 5] = {
    0x00,0x00,0x00,0x00,0x00, 0x00,0x00,0x5F,0x00,0x00,
    0x00,0x07,0x00,0x07,0x00, 0x14,0x7F,0x14,0x7F,0x14,
    0x24,0x2A,0x7F,0x2A,0x12, 0x23,0x13,0x08,0x64,0x62,
    0x36,0x49,0x55,0x22,0x50, 0x00,0x05,0x03,0x00,0x00,
    0x00,0x1C,0x22,0x41,0x00, 0x00,0x41,0x22,0x1C,0x00,
    0x14,0x08,0x3E,0x08,0x14, 0x08,0x08,0x3E,0x08,0x08,
    0x00,0x50,0x30,0x00,0x00, 0x08,0x08,0x08,0x08,0x08,
    0x00,0x60,0x60,0x00,0x00, 0x20,0x10,0x08,0x04,0x02,
    0x3E,0x51,0x49,0x45,0x3E, 0x00,0x42,0x7F,0x40,0x00,
    0x42,0x61,0x51,0x49,0x46, 0x21,0x41,0x45,0x4B,0x31,
    0x18,0x14,0x12,0x7F,0x10, 0x27,0x45,0x45,0x45,0x39,
    0x3C,0x4A,0x49,0x49,0x30, 0x01,0x71,0x09,0x05,0x03,
    0x36,0x49,0x49,0x49,0x36, 0x06,0x49,0x49,0x29,0x1E,
    0x00,0x36,0x36,0x00,0x00, 0x00,0x56,0x36,0x00,0x00,
    0x08,0x14,0x22,0x41,0x00, 0x14,0x14,0x14,0x14,0x14,
    0x00,0x41,0x22,0x14,0x08, 0x02,0x01,0x51,0x09,0x06,
    0x32,0x49,0x79,0x41,0x3E, 0x7E,0x11,0x11,0x11,0x7E,
    0x7F,0x49,0x49,0x49,0x36, 0x3E,0x41,0x41,0x41,0x22,
    0x7F,0x41,0x41,0x22,0x1C, 0x7F,0x49,0x49,0x49,0x41,
    0x7F,0x09,0x09,0x09,0x01, 0x3E,0x41,0x49,0x49,0x7A,
    0x7F,0x08,0x08,0x08,0x7F, 0x00,0x41,0x7F,0x41,0x00,
    0x20,0x40,0x41,0x3F,0x01, 0x7F,0x08,0x14,0x22,0x41,
    0x7F,0x40,0x40,0x40,0x40, 0x7F,0x02,0x0C,0x02,0x7F,
    0x7F,0x04,0x08,0x10,0x7F, 0x3E,0x41,0x41,0x41,0x3E,
    0x7F,0x09,0x09,0x09,0x06, 0x3E,0x41,0x51,0x21,0x5E,
    0x7F,0x09,0x19,0x29,0x46, 0x46,0x49,0x49,0x49,0x31,
    0x01,0x01,0x7F,0x01,0x01, 0x3F,0x40,0x40,0x40,0x3F,
    0x1F,0x20,0x40,0x20,0x1F, 0x3F,0x40,0x38,0x40,0x3F,
    0x63,0x14,0x08,0x14,0x63, 0x07,0x08,0x70,0x08,0x07,
    0x61,0x51,0x49,0x45,0x43, 0x00,0x7F,0x41,0x41,0x00,
    0x02,0x04,0x08,0x10,0x20, 0x00,0x41,0x41,0x7F,0x00,
    0x04,0x02,0x01,0x02,0x04, 0x40,0x40,0x40,0x40,0x40,
    0x00,0x01,0x02,0x04,0x00, 0x20,0x54,0x54,0x54,0x78,
    0x7F,0x48,0x44,0x44,0x38, 0x38,0x44,0x44,0x44,0x20,
    0x38,0x44,0x44,0x48,0x7F, 0x38,0x54,0x54,0x54,0x18,
    0x08,0x7E,0x09,0x01,0x02, 0x0C,0x52,0x52,0x52,0x3E,
    0x7F,0x08,0x04,0x04,0x78, 0x00,0x44,0x7D,0x40,0x00,
    0x20,0x40,0x44,0x3D,0x00, 0x7F,0x10,0x28,0x44,0x00,
    0x00,0x41,0x7F,0x40,0x00, 0x7C,0x04,0x18,0x04,0x78,
    0x7C,0x08,0x04,0x04,0x78, 0x38,0x44,0x44,0x44,0x38,
    0x7C,0x14,0x14,0x14,0x08, 0x08,0x14,0x14,0x18,0x7C,
    0x7C,0x08,0x04,0x04,0x08, 0x48,0x54,0x54,0x54,0x20,
    0x04,0x3F,0x44,0x40,0x20, 0x3C,0x40,0x40,0x20,0x7C,
    0x1C,0x20,0x40,0x20,0x1C, 0x3C,0x40,0x30,0x40,0x3C,
    0x44,0x28,0x10,0x28,0x44, 0x0C,0x50,0x50,0x50,0x3C,
    0x44,0x64,0x54,0x4C,0x44, 0x00,0x08,0x36,0x41,0x00,
    0x00,0x00,0x7F,0x00,0x00, 0x00,0x41,0x36,0x08,0x00,
    0x08,0x04,0x08,0x10,0x08
};

/* ---------------------------------------------------------------------------
 *  Global state
 * ------------------------------------------------------------------------- */

typedef struct {
    const char *name;
    float wx;         /* world X, metres */
    float wy;         /* world Y, metres */
    float bearingDeg; /* compass bearing toward the signal, 0 = north */
    float db;         /* received level in dBm */
} Waypoint;

static Waypoint g_waypoints[NUM_WAYPOINTS] = {
    { "ALPHA",   140.0f, 820.0f, 133.0f, -71.0f },
    { "BRAVO",   830.0f, 780.0f, 229.0f, -66.0f },
    { "CHARLIE", 200.0f, 220.0f,  48.0f, -80.0f },
    { "DELTA",   760.0f, 180.0f, 322.0f, -75.0f },
    { "ECHO",    500.0f, 930.0f, 179.0f, -62.0f },
    { "FOXTROT", 480.0f,  70.0f,   4.0f, -88.0f }
};

typedef struct {
    float wx;
    float wy;
    float rmsErrorDeg;
} Candidate;

static Candidate g_candidates[MAX_CANDIDATES];
static int       g_numCandidates = 0;

/* Map view: fits the WORLD_SIZE square inside the centre panel. */
typedef struct { float screenX, screenY, sizePx; } MapView;
static MapView g_map;

/* Vector field texture */
static GLuint g_vectorTexture = 0;
static int    g_vectorTextureAllocated = 0;
static unsigned char g_vectorPixels[VEC_TEX_SIZE * VEC_TEX_SIZE * 3];

/* Dirty flags */
static int g_candidatesDirty = 1;
static int g_vectorDirty     = 1;

/* Slider drag state */
static int    g_activeWaypoint = -1;  /* -1 = none */
static int    g_activeSlider   = -1;  /* 0 = bearing, 1 = dB, -1 = none */
static double g_mouseX = 0.0;
static double g_mouseY = 0.0;

/* ---------------------------------------------------------------------------
 *  Basic geometry helpers
 * ------------------------------------------------------------------------- */

typedef struct { float x, y, w, h; } Rect;

static int pointInRect(float px, float py, Rect r)
{
    return (px >= r.x && px <= r.x + r.w &&
            py >= r.y && py <= r.y + r.h);
}

static MapView makeMapView(void)
{
    const float pad = 12.0f;
    float availW = CENTER_PANEL_W - pad * 2.0f;      /* 656 */
    float availH = (float)WIN_H   - pad * 2.0f;      /* 776 */
    float side   = (availW < availH) ? availW : availH;

    MapView v;
    v.sizePx  = side;
    v.screenX = CENTER_PANEL_X + (CENTER_PANEL_W - side) * 0.5f;
    v.screenY = (float)WIN_H * 0.5f - side * 0.5f;
    return v;
}

static float worldToScreenX(float wx)
{
    return g_map.screenX + (wx / WORLD_SIZE) * g_map.sizePx;
}

static float worldToScreenY(float wy)
{
    /* World Y increases north; screen Y increases downward. */
    return g_map.screenY + g_map.sizePx - (wy / WORLD_SIZE) * g_map.sizePx;
}

/* ---------------------------------------------------------------------------
 *  Drawing primitives
 * ------------------------------------------------------------------------- */

static void fillRect(float x, float y, float w, float h,
                     float r, float g, float b, float a)
{
    glColor4f(r, g, b, a);
    glBegin(GL_QUADS);
        glVertex2f(x,     y);
        glVertex2f(x + w, y);
        glVertex2f(x + w, y + h);
        glVertex2f(x,     y + h);
    glEnd();
}

static void strokeRect(float x, float y, float w, float h,
                       float r, float g, float b, float a)
{
    glColor4f(r, g, b, a);
    glBegin(GL_LINE_LOOP);
        glVertex2f(x,     y);
        glVertex2f(x + w, y);
        glVertex2f(x + w, y + h);
        glVertex2f(x,     y + h);
    glEnd();
}

static void fillCircle(float cx, float cy, float radius, int segments,
                       float r, float g, float b, float a)
{
    int i;
    glColor4f(r, g, b, a);
    glBegin(GL_TRIANGLE_FAN);
        glVertex2f(cx, cy);
        for (i = 0; i <= segments; ++i) {
            float ang = (float)i / (float)segments * 2.0f * PI_F;
            glVertex2f(cx + cosf(ang) * radius, cy + sinf(ang) * radius);
        }
    glEnd();
}

static void strokeCircle(float cx, float cy, float radius, int segments,
                         float r, float g, float b, float a)
{
    int i;
    glColor4f(r, g, b, a);
    glBegin(GL_LINE_LOOP);
        for (i = 0; i < segments; ++i) {
            float ang = (float)i / (float)segments * 2.0f * PI_F;
            glVertex2f(cx + cosf(ang) * radius, cy + sinf(ang) * radius);
        }
    glEnd();
}

static float textWidth(const char *text, float scale)
{
    return (float)strlen(text) * 6.0f * scale;
}

static void drawText(float x, float y, float scale,
                     const char *text, float r, float g, float b)
{
    const char *p;
    float penX = x;

    glColor4f(r, g, b, 1.0f);
    glBegin(GL_QUADS);
    for (p = text; *p != '\0'; ++p) {
        unsigned char ch = (unsigned char)(*p);
        const unsigned char *glyph;
        int col, row;

        if (ch < 0x20 || ch > 0x7E) ch = '?';
        glyph = &FONT5X7[(ch - 0x20) * 5];

        for (col = 0; col < 5; ++col) {
            unsigned char bits = glyph[col];
            for (row = 0; row < 7; ++row) {
                if (bits & (1u << row)) {
                    float px = penX + (float)col * scale;
                    float py = y    + (float)row * scale;
                    glVertex2f(px,         py);
                    glVertex2f(px + scale, py);
                    glVertex2f(px + scale, py + scale);
                    glVertex2f(px,         py + scale);
                }
            }
        }
        penX += 6.0f * scale;
    }
    glEnd();
}

static void drawTextF(float x, float y, float scale,
                      float r, float g, float b, const char *fmt, ...)
{
    char buf[256];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    drawText(x, y, scale, buf, r, g, b);
}

static void drawTextRight(float rightX, float y, float scale,
                          const char *text, float r, float g, float b)
{
    drawText(rightX - textWidth(text, scale), y, scale, text, r, g, b);
}

static void hsvToRgb(float h, float s, float v,
                     float *outR, float *outG, float *outB)
{
    float hh;
    int sector;
    float f, p, q, t;

    h = h - floorf(h);
    hh = h * 6.0f;
    sector = (int)hh;
    f = hh - (float)sector;

    p = v * (1.0f - s);
    q = v * (1.0f - s * f);
    t = v * (1.0f - s * (1.0f - f));

    switch (sector % 6) {
        case 0:  *outR = v; *outG = t; *outB = p; break;
        case 1:  *outR = q; *outG = v; *outB = p; break;
        case 2:  *outR = p; *outG = v; *outB = t; break;
        case 3:  *outR = p; *outG = q; *outB = v; break;
        case 4:  *outR = t; *outG = p; *outB = v; break;
        default: *outR = v; *outG = p; *outB = q; break;
    }
}

/* ---------------------------------------------------------------------------
 *  Candidate computation
 * ------------------------------------------------------------------------- */

static float angularDiff(float a, float b)
{
    float d = a - b;
    while (d >  180.0f) d -= 360.0f;
    while (d < -180.0f) d += 360.0f;
    return d;
}

static float rmsErrorAt(float cx, float cy)
{
    float sumSq = 0.0f;
    int i;

    for (i = 0; i < NUM_WAYPOINTS; ++i) {
        float dx = cx - g_waypoints[i].wx;
        float dy = cy - g_waypoints[i].wy;
        float dist2 = dx*dx + dy*dy;
        float bearing, diff;

        if (dist2 < 0.01f) {
            sumSq += 1.0e6f;
            continue;
        }
        /* atan2(east, north) gives compass bearing in degrees (-180..180). */
        bearing = atan2f(dx, dy) * 180.0f / PI_F;
        diff = angularDiff(bearing, g_waypoints[i].bearingDeg);
        sumSq += diff * diff;
    }
    return sqrtf(sumSq / (float)NUM_WAYPOINTS);
}

static void computeCandidates(void)
{
    const int   G    = 64;
    const float step = WORLD_SIZE / (float)G;
    const float MIN_SEPARATION = 80.0f;
    int gx, gy, ci;

    g_numCandidates = 0;

    while (g_numCandidates < MAX_CANDIDATES) {
        float bestScore = 1.0e9f;
        float bestWx = 0.0f, bestWy = 0.0f;

        for (gy = 0; gy < G; ++gy) {
            for (gx = 0; gx < G; ++gx) {
                float wx = ((float)gx + 0.5f) * step;
                float wy = ((float)gy + 0.5f) * step;
                int   skip = 0;
                float score;

                for (ci = 0; ci < g_numCandidates; ++ci) {
                    float ddx = wx - g_candidates[ci].wx;
                    float ddy = wy - g_candidates[ci].wy;
                    if (ddx*ddx + ddy*ddy < MIN_SEPARATION * MIN_SEPARATION) {
                        skip = 1;
                        break;
                    }
                }
                if (skip) continue;

                score = rmsErrorAt(wx, wy);
                if (score < bestScore) {
                    bestScore = score;
                    bestWx    = wx;
                    bestWy    = wy;
                }
            }
        }

        if (bestScore > 1.0e8f) break; /* nothing left */

        g_candidates[g_numCandidates].wx          = bestWx;
        g_candidates[g_numCandidates].wy          = bestWy;
        g_candidates[g_numCandidates].rmsErrorDeg = bestScore;
        g_numCandidates++;

        /* Stop after the first candidate if the fit is already mediocre. */
        if (bestScore > 30.0f) break;
    }
}

/* ---------------------------------------------------------------------------
 *  Vector field texture
 *
 *  For each texel we compute the bearing from that ground point toward the
 *  best candidate. The hue of the texel is that bearing. Stretched over the
 *  map, the result is a compass-rose style colour field.
 * ------------------------------------------------------------------------- */

static void updateVectorTexture(void)
{
    int haveSource = (g_numCandidates > 0);
    float srcX = haveSource ? g_candidates[0].wx : 0.0f;
    float srcY = haveSource ? g_candidates[0].wy : 0.0f;
    int tx, ty;

    for (ty = 0; ty < VEC_TEX_SIZE; ++ty) {
        for (tx = 0; tx < VEC_TEX_SIZE; ++tx) {
            float wx = ((float)tx + 0.5f) / (float)VEC_TEX_SIZE * WORLD_SIZE;
            float wy = ((float)ty + 0.5f) / (float)VEC_TEX_SIZE * WORLD_SIZE;
            unsigned char *px = &g_vectorPixels[(ty * VEC_TEX_SIZE + tx) * 3];

            if (!haveSource) {
                px[0] = 25; px[1] = 25; px[2] = 32;
            } else {
                float dx = srcX - wx;
                float dy = srcY - wy;
                float dist = sqrtf(dx*dx + dy*dy);
                float bearing, hue, sat, val;
                float r, g, b;

                if (dist < 1.0f) dist = 1.0f;
                bearing = atan2f(dx, dy) * 180.0f / PI_F;
                if (bearing < 0.0f) bearing += 360.0f;

                hue = bearing / 360.0f;
                sat = 0.85f;
                val = 0.35f + 0.65f / (1.0f + dist / 400.0f);

                hsvToRgb(hue, sat, val, &r, &g, &b);
                px[0] = (unsigned char)(r * 255.0f);
                px[1] = (unsigned char)(g * 255.0f);
                px[2] = (unsigned char)(b * 255.0f);
            }
        }
    }

    if (!g_vectorTextureAllocated) {
        glGenTextures(1, &g_vectorTexture);
        glBindTexture(GL_TEXTURE_2D, g_vectorTexture);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        g_vectorTextureAllocated = 1;
    }

    glBindTexture(GL_TEXTURE_2D, g_vectorTexture);
    glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGB,
                 VEC_TEX_SIZE, VEC_TEX_SIZE, 0,
                 GL_RGB, GL_UNSIGNED_BYTE, g_vectorPixels);
}

/* ---------------------------------------------------------------------------
 *  Slider helpers
 * ------------------------------------------------------------------------- */

static Rect waypointBearingSliderRect(int i)
{
    float baseY = 45.0f + (float)i * 115.0f;
    Rect r;
    r.x = 15.0f;
    r.y = baseY + 28.0f;
    r.w = LEFT_PANEL_W - 30.0f;
    r.h = 16.0f;
    return r;
}

static Rect waypointDbSliderRect(int i)
{
    float baseY = 45.0f + (float)i * 115.0f;
    Rect r;
    r.x = 15.0f;
    r.y = baseY + 72.0f;
    r.w = LEFT_PANEL_W - 30.0f;
    r.h = 16.0f;
    return r;
}

static float sliderFractionFromMouse(Rect r, double mouseX)
{
    float f = ((float)mouseX - r.x - 2.0f) / (r.w - 4.0f);
    if (f < 0.0f) f = 0.0f;
    if (f > 1.0f) f = 1.0f;
    return f;
}

static void drawSlider(Rect r, float fraction,
                       float fillR, float fillG, float fillB)
{
    float thumbX;
    float fillW = (r.w - 4.0f) * fraction;

    /* Track */
    fillRect(r.x, r.y, r.w, r.h, 0.14f, 0.14f, 0.17f, 1.0f);
    strokeRect(r.x, r.y, r.w, r.h, 0.30f, 0.30f, 0.36f, 1.0f);

    /* Fill */
    if (fillW > 0.0f) {
        fillRect(r.x + 2.0f, r.y + 2.0f, fillW, r.h - 4.0f,
                 fillR, fillG, fillB, 1.0f);
    }

    /* Thumb */
    thumbX = r.x + 2.0f + fillW;
    if (thumbX < r.x + 4.0f) thumbX = r.x + 4.0f;
    if (thumbX > r.x + r.w - 4.0f) thumbX = r.x + r.w - 4.0f;
    fillCircle(thumbX, r.y + r.h * 0.5f, 7.0f, 10,
               0.95f, 0.95f, 1.0f, 1.0f);
}

/* ---------------------------------------------------------------------------
 *  Input callbacks
 * ------------------------------------------------------------------------- */

static void onMouseButton(GLFWwindow *w, int button, int action, int mods)
{
    int i;
    (void)w; (void)mods;

    if (button != GLFW_MOUSE_BUTTON_LEFT) return;

    if (action == GLFW_PRESS) {
        for (i = 0; i < NUM_WAYPOINTS; ++i) {
            Rect rb = waypointBearingSliderRect(i);
            Rect rd = waypointDbSliderRect(i);
            if (pointInRect((float)g_mouseX, (float)g_mouseY, rb)) {
                g_activeWaypoint = i;
                g_activeSlider   = 0;
                return;
            }
            if (pointInRect((float)g_mouseX, (float)g_mouseY, rd)) {
                g_activeWaypoint = i;
                g_activeSlider   = 1;
                return;
            }
        }
    } else if (action == GLFW_RELEASE) {
        g_activeWaypoint = -1;
        g_activeSlider   = -1;
    }
}

static void onCursorPos(GLFWwindow *w, double xpos, double ypos)
{
    (void)w;
    g_mouseX = xpos;
    g_mouseY = ypos;

    if (g_activeWaypoint < 0) return;

    if (g_activeSlider == 0) {
        Rect r = waypointBearingSliderRect(g_activeWaypoint);
        float f = sliderFractionFromMouse(r, xpos);
        g_waypoints[g_activeWaypoint].bearingDeg = f * 360.0f;
        g_candidatesDirty = 1;
    } else if (g_activeSlider == 1) {
        Rect r = waypointDbSliderRect(g_activeWaypoint);
        float f = sliderFractionFromMouse(r, xpos);
        g_waypoints[g_activeWaypoint].db = -100.0f + f * 60.0f;
        /* dB does not affect triangulation, only the label */
    }
}

static void onKey(GLFWwindow *w, int key, int scancode, int action, int mods)
{
    (void)scancode; (void)mods;
    if (action != GLFW_PRESS) return;
    if (key == GLFW_KEY_ESCAPE) {
        glfwSetWindowShouldClose(w, GLFW_TRUE);
    }
}

/* ---------------------------------------------------------------------------
 *  Panel drawing
 * ------------------------------------------------------------------------- */

static void drawLeftPanel(void)
{
    int i;

    /* Background */
    fillRect(LEFT_PANEL_X, 0.0f, LEFT_PANEL_W, (float)WIN_H,
             0.09f, 0.09f, 0.11f, 1.0f);

    /* Title */
    drawTextF(15.0f, 14.0f, 2.0f, 0.85f, 0.85f, 0.95f, "WAYPOINTS");

    for (i = 0; i < NUM_WAYPOINTS; ++i) {
        Waypoint *wp = &g_waypoints[i];
        Rect rb = waypointBearingSliderRect(i);
        Rect rd = waypointDbSliderRect(i);
        float baseY = 45.0f + (float)i * 115.0f;
        float bFrac = wp->bearingDeg / 360.0f;
        float dFrac = (wp->db + 100.0f) / 60.0f;
        float br, bg, bb;
        float dr, dg, db;

        /* Row separator */
        if (i > 0) {
            fillRect(15.0f, baseY - 6.0f, LEFT_PANEL_W - 30.0f, 1.0f,
                     0.18f, 0.18f, 0.22f, 1.0f);
        }

        /* Header line */
        drawText(baseY + 4.0f, 1.6f, wp->name,
                 0.95f, 0.95f, 1.0f, 1.0f);
        drawTextF(165.0f, baseY + 4.0f, 1.5f,
                  0.75f, 0.80f, 0.85f,
                  "%3.0f deg  %+4.0f dB",
                  wp->bearingDeg, wp->db);

        /* Bearing slider */
        drawTextF(15.0f, baseY + 18.0f, 1.4f,
                  0.65f, 0.70f, 0.75f, "Bearing");
        hsvToRgb(bFrac, 0.75f, 0.85f, &br, &bg, &bb);
        drawSlider(rb, bFrac, br, bg, bb);

        /* dB slider */
        drawTextF(15.0f, baseY + 62.0f, 1.4f,
                  0.65f, 0.70f, 0.75f, "Signal dBm");
        hsvToRgb(0.33f * (1.0f - dFrac), 0.75f, 0.85f, &dr, &dg, &db);
        drawSlider(rd, dFrac, dr, dg, db);
    }

    /* Footer */
    drawTextF(15.0f, (float)WIN_H - 30.0f, 1.3f,
              0.50f, 0.52f, 0.58f, "Drag sliders to adjust bearings.");
    drawTextF(15.0f, (float)WIN_H - 16.0f, 1.3f,
              0.50f, 0.52f, 0.58f, "ESC to quit.");
}

static void drawCenterPanel(void)
{
    int i;
    int x, y;

    /* Background behind the map */
    fillRect(CENTER_PANEL_X, 0.0f, CENTER_PANEL_W, (float)WIN_H,
             0.06f, 0.06f, 0.08f, 1.0f);

    /* Vector field texture (map background) */
    glEnable(GL_TEXTURE_2D);
    glBindTexture(GL_TEXTURE_2D, g_vectorTexture);
    glColor4f(1.0f, 1.0f, 1.0f, 1.0f);
    glBegin(GL_QUADS);
        glTexCoord2f(0.0f, 0.0f);
        glVertex2f(g_map.screenX, g_map.screenY + g_map.sizePx);
        glTexCoord2f(1.0f, 0.0f);
        glVertex2f(g_map.screenX + g_map.sizePx, g_map.screenY + g_map.sizePx);
        glTexCoord2f(1.0f, 1.0f);
        glVertex2f(g_map.screenX + g_map.sizePx, g_map.screenY);
        glTexCoord2f(0.0f, 1.0f);
        glVertex2f(g_map.screenX, g_map.screenY);
    glEnd();
    glDisable(GL_TEXTURE_2D);

    /* Subtle grid every 100 m */
    glColor4f(0.0f, 0.0f, 0.0f, 0.25f);
    glBegin(GL_LINES);
    for (i = 0; i <= 10; ++i) {
        float m = (float)i * 100.0f;
        float sx = worldToScreenX(m);
        float sy = worldToScreenY(m);
        glVertex2f(sx, g_map.screenY);
        glVertex2f(sx, g_map.screenY + g_map.sizePx);
        glVertex2f(g_map.screenX, sy);
        glVertex2f(g_map.screenX + g_map.sizePx, sy);
    }
    glEnd();

    /* Clip everything else to the map rect */
    glEnable(GL_SCISSOR_TEST);
    glScissor((GLint)g_map.screenX,
              (GLint)(WIN_H - (g_map.screenY + g_map.sizePx)),
              (GLsizei)g_map.sizePx,
              (GLsizei)g_map.sizePx);

    /* Bearing rays from each waypoint */
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    glLineWidth(2.0f);
    for (i = 0; i < NUM_WAYPOINTS; ++i) {
        Waypoint *wp = &g_waypoints[i];
        float sx = worldToScreenX(wp->wx);
        float sy = worldToScreenY(wp->wy);
        float rad = wp->bearingDeg * PI_F / 180.0f;
        float dx = sinf(rad);
        float dy = -cosf(rad);
        float len = g_map.sizePx * 2.0f;
        float cr, cg, cb;

        hsvToRgb(wp->bearingDeg / 360.0f, 0.7f, 0.9f, &cr, &cg, &cb);

        glColor4f(cr, cg, cb, 0.65f);
        glBegin(GL_LINES);
            glVertex2f(sx, sy);
            glVertex2f(sx + dx * len, sy + dy * len);
        glEnd();
    }
    glLineWidth(1.0f);

    /* Waypoint markers */
    for (i = 0; i < NUM_WAYPOINTS; ++i) {
        Waypoint *wp = &g_waypoints[i];
        float sx = worldToScreenX(wp->wx);
        float sy = worldToScreenY(wp->wy);
        fillRect(sx - 4.0f, sy - 4.0f, 8.0f, 8.0f,
                 1.0f, 1.0f, 1.0f, 1.0f);
        drawText(sx + 7.0f, sy - 5.0f, 1.2f,
                 wp->name, 1.0f, 1.0f, 1.0f);
    }

    /* Candidate markers */
    for (i = 0; i < g_numCandidates; ++i) {
        Candidate *c = &g_candidates[i];
        float sx = worldToScreenX(c->wx);
        float sy = worldToScreenY(c->wy);
        float r, g, b;

        if (i == 0) {
            r = 1.0f; g = 0.35f; b = 0.35f;
        } else {
            r = 0.90f; g = 0.75f; b = 0.30f;
        }

        strokeCircle(sx, sy, 12.0f, 24, r, g, b, 1.0f);
        strokeCircle(sx, sy,  6.0f, 18, r, g, b, 1.0f);
        fillCircle  (sx, sy,  2.5f, 10, r, g, b, 1.0f);
    }

    glDisable(GL_BLEND);
    glDisable(GL_SCISSOR_TEST);

    /* Map border and compass labels (outside the clip) */
    strokeRect(g_map.screenX, g_map.screenY,
               g_map.sizePx, g_map.sizePx,
               0.55f, 0.55f, 0.60f, 1.0f);

    drawTextF(g_map.screenX + g_map.sizePx * 0.5f - 6.0f,
              g_map.screenY - 14.0f, 1.4f,
              0.80f, 0.80f, 0.85f, "N");
    drawTextF(g_map.screenX + g_map.sizePx * 0.5f - 6.0f,
              g_map.screenY + g_map.sizePx + 2.0f, 1.4f,
              0.80f, 0.80f, 0.85f, "S");
    drawTextF(g_map.screenX + 4.0f,
              g_map.screenY + g_map.sizePx * 0.5f - 5.0f, 1.4f,
              0.80f, 0.80f, 0.85f, "W");
    drawTextF(g_map.screenX + g_map.sizePx - 12.0f,
              g_map.screenY + g_map.sizePx * 0.5f - 5.0f, 1.4f,
              0.80f, 0.80f, 0.85f, "E");

    (void)x; (void)y;
}

static void drawRightPanel(void)
{
    int i;

    fillRect(RIGHT_PANEL_X, 0.0f, RIGHT_PANEL_W, (float)WIN_H,
             0.09f, 0.09f, 0.11f, 1.0f);

    drawTextF(RIGHT_PANEL_X + 15.0f, 14.0f, 2.0f,
              0.85f, 0.85f, 0.95f, "CANDIDATES");

    if (g_numCandidates == 0) {
        drawTextF(RIGHT_PANEL_X + 15.0f, 60.0f, 1.4f,
                  0.70f, 0.70f, 0.75f, "No candidates.");
        drawTextF(RIGHT_PANEL_X + 15.0f, 80.0f, 1.4f,
                  0.70f, 0.70f, 0.75f, "Adjust waypoints.");
        return;
    }

    for (i = 0; i < g_numCandidates; ++i) {
        Candidate *c = &g_candidates[i];
        float baseY = 60.0f + (float)i * 110.0f;
        float quality;
        float qr, qg, qb;

        /* Row separator */
        if (i > 0) {
            fillRect(RIGHT_PANEL_X + 15.0f, baseY - 8.0f,
                     RIGHT_PANEL_W - 30.0f, 1.0f,
                     0.18f, 0.18f, 0.22f, 1.0f);
        }

        /* Rank label */
        drawTextF(RIGHT_PANEL_X + 15.0f, baseY + 4.0f, 1.8f,
                  (i == 0) ? 1.0f : 0.85f,
                  (i == 0) ? 0.65f : 0.80f,
                  (i == 0) ? 0.45f : 0.55f,
                  "#%d%s", i + 1, (i == 0) ? " BEST" : "");

        /* Position */
        drawTextF(RIGHT_PANEL_X + 15.0f, baseY + 28.0f, 1.5f,
                  0.75f, 0.78f, 0.85f,
                  "X: %4.0f m", c->wx);
        drawTextF(RIGHT_PANEL_X + 15.0f, baseY + 46.0f, 1.5f,
                  0.75f, 0.78f, 0.85f,
                  "Y: %4.0f m", c->wy);

        /* RMS error with colour-coded quality */
        quality = c->rmsErrorDeg;
        if (quality < 5.0f)       { qr = 0.35f; qg = 0.95f; qb = 0.45f; }
        else if (quality < 15.0f) { qr = 0.85f; qg = 0.90f; qb = 0.35f; }
        else if (quality < 30.0f) { qr = 0.95f; qg = 0.65f; qb = 0.30f; }
        else                      { qr = 0.95f; qg = 0.30f; qb = 0.30f; }

        drawTextF(RIGHT_PANEL_X + 15.0f, baseY + 68.0f, 1.5f,
                  qr, qg, qb,
                  "RMS: %.1f deg", c->rmsErrorDeg);
    }

    /* Footer summary */
    drawTextF(RIGHT_PANEL_X + 15.0f, (float)WIN_H - 30.0f, 1.3f,
              0.50f, 0.52f, 0.58f,
              "%d candidate location%s found.",
              g_numCandidates, (g_numCandidates == 1) ? "" : "s");
}

/* ---------------------------------------------------------------------------
 *  Main
 * ------------------------------------------------------------------------- */

int main(void)
{
    GLFWwindow *win;
    double lastTime;

    if (!glfwInit()) {
        fprintf(stderr, "glfwInit failed\n");
        return 1;
    }

    /* Old hardware: no MSAA, no resizable window, compat profile. */
    glfwWindowHint(GLFW_RESIZABLE, GLFW_FALSE);
    glfwWindowHint(GLFW_SAMPLES, 0);
    glfwWindowHint(GLFW_DEPTH_BITS, 0);
    glfwWindowHint(GLFW_STENCIL_BITS, 0);

    win = glfwCreateWindow(WIN_W, WIN_H, "Radio Signal Mapper", NULL, NULL);
    if (!win) {
        fprintf(stderr, "glfwCreateWindow failed\n");
        glfwTerminate();
        return 1;
    }

    glfwMakeContextCurrent(win);
    glfwSwapInterval(1); /* vsync */

    glfwSetMouseButtonCallback(win, onMouseButton);
    glfwSetCursorPosCallback(win, onCursorPos);
    glfwSetKeyCallback(win, onKey);

    /* Fixed 2D projection, top-left origin. */
    glMatrixMode(GL_PROJECTION);
    glLoadIdentity();
    glOrtho(0.0, (double)WIN_W, (double)WIN_H, 0.0, -1.0, 1.0);
    glMatrixMode(GL_MODELVIEW);
    glLoadIdentity();

    glDisable(GL_DEPTH_TEST);
    glDisable(GL_CULL_FACE);
    glDisable(GL_LIGHTING);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

    g_map = makeMapView();

    lastTime = glfwGetTime();
    while (!glfwWindowShouldClose(win)) {
        double now = glfwGetTime();
        double dt  = now - lastTime;
        lastTime = now;
        (void)dt;

        if (g_candidatesDirty) {
            computeCandidates();
            g_candidatesDirty = 0;
            g_vectorDirty = 1;
        }
        if (g_vectorDirty) {
            updateVectorTexture();
            g_vectorDirty = 0;
        }

        glClearColor(0.04f, 0.04f, 0.05f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT);

        drawLeftPanel();
        drawCenterPanel();
        drawRightPanel();

        glfwSwapBuffers(win);
        glfwPollEvents();
    }

    if (g_vectorTextureAllocated) {
        glDeleteTextures(1, &g_vectorTexture);
    }
    glfwDestroyWindow(win);
    glfwTerminate();
    return 0;
}
