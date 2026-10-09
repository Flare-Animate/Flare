// Moho xsheet plan: layers flatten to columns, switches gate rows, bones inherit.
#include "MohoPlan.h"
#include <cstdio>
static int g_fail = 0;
static void check(bool ok, const char *w) {
    printf("%s %s\n", ok ? "  ok  " : "  FAIL", w);
    if (!ok) ++g_fail;
}
static Moho::Layer L(const char *n, const char *t, int bone = -2) {
    Moho::Layer l; l.name = n; l.type = t; l.parentBone = bone; return l;
}
int main() {
    Moho::Document d;
    d.startFrame = 1; d.endFrame = 10;
    Moho::Layer img = L("bg", "ImageLayer"); img.imagePath = "images/bg.png";
    Moho::Layer rig = L("Rig", "BoneLayer");
    rig.children << L("arm", "MeshLayer", 2);
    Moho::Layer head = L("Head", "GroupLayer", 0);
    head.children << L("face", "MeshLayer");
    rig.children << head;
    Moho::Layer mouth = L("Mouth", "SwitchLayer");
    mouth.children << L("A", "MeshLayer") << L("O", "MeshLayer");
    rig.children << mouth;
    rig.children << L("emptyGroup", "GroupLayer");
    d.layers << img << rig;
    Moho::Switch sw; sw.name = "Mouth"; sw.alternatives << "A" << "O";
    sw.childAtRest = "A";
    sw.keys.frames << 0 << 5;
    sw.keys.values << QJsonValue("A") << QJsonValue("O");
    d.switches << sw;

    const Moho::Plan p = Moho::makePlan(d);
    check(p.rowCount == 10 && p.firstFrame == 1, "row range from frame range");
    check(p.columns.size() == 5, "every drawable leaf is a column, empty group skipped");
    check(p.columns[0].imagePath == "images/bg.png", "image layer keeps bitmap");
    check(p.columns[1].name == "Rig/arm" && p.columns[1].parentBone == 2, "bone binding kept");
    check(p.columns[2].name == "Rig/Head/face" && p.columns[2].parentBone == 0, "group bone inherited");
    const Moho::ColumnPlan &a = p.columns[3], &o = p.columns[4];
    check(a.switchName == "Mouth" && o.switchName == "Mouth", "switch children tagged");
    check(a.rows[0] && a.rows[3] && !a.rows[4] && !a.rows[9], "A shows frames 1-4");
    check(!o.rows[0] && o.rows[4] && o.rows[9], "O shows from frame 5");
    Moho::Switch k; k.alternatives << "X";
    check(Moho::switchChildAt(k, 3) == "X", "keyless switch falls back to first child");
    printf(g_fail ? "FAILED %d\n" : "all passed\n", g_fail);
    return g_fail ? 1 : 0;
}
