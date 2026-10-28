#pragma once
// MohoPlan.h - turn a parsed Moho::Document into an xsheet layout plan.
// Header-only and Qt-only so it is unit tested without the app
// (tests/native/moho_plan_tests.cpp). mohoimport.cpp executes the plan.

#include "MohoReader.h"

namespace Moho {

struct ColumnPlan {
    QString name;        // "Group/Child" path, for the column name
    QString type;        // MeshLayer | ImageLayer | ...
    QString imagePath;   // ImageLayer bitmap (or mesh texture), project relative
    int parentBone = -2; // -2 none, -1 region bound, >=0 bone index
    bool visible = true;
    QString switchName;  // set when the layer is a switch alternative
    QVector<bool> rows;  // per xsheet row: does the layer show? (switch keys)
};

struct Plan {
    int firstFrame = 1;  // Moho frame mapped to xsheet row 0
    int rowCount = 1;
    QVector<ColumnPlan> columns;  // bottom-to-top draw order
};

// Active switch child at Moho frame f: last key at or before f (step keys).
inline QString switchChildAt(const Switch &sw, int f) {
    QString cur = sw.childAtRest;
    const int n = sw.keys.size();
    for (int i = 0; i < n; ++i)
        if (sw.keys.frames[i] <= f) cur = sw.keys.values[i].toString();
    if (cur.isEmpty() && !sw.alternatives.isEmpty()) cur = sw.alternatives.first();
    return cur;
}

namespace detail {
inline void flatten(const Document &doc, const Layer &l, const QString &prefix,
                    const Switch *sw, int inheritBone, Plan &p) {
    const QString path = prefix.isEmpty() ? l.name : prefix + "/" + l.name;
    const int bone = l.parentBone != -2 ? l.parentBone : inheritBone;
    if (!l.children.isEmpty()) {
        const Switch *mySw = nullptr;
        if (l.type == "SwitchLayer")
            for (const Switch &s : doc.switches)
                if (s.name == l.name) { mySw = &s; break; }
        for (const Layer &c : l.children) {
            // Child of a switch: tag it, then descend without re-tagging.
            Plan sub;
            sub.firstFrame = p.firstFrame;
            sub.rowCount = p.rowCount;
            flatten(doc, c, path, mySw ? mySw : sw, bone, sub);
            for (ColumnPlan &cp : sub.columns) {
                if (mySw && cp.switchName.isEmpty()) {
                    cp.switchName = mySw->name;
                    for (int r = 0; r < p.rowCount; ++r)
                        cp.rows[r] = cp.rows[r] &&
                                     switchChildAt(*mySw, p.firstFrame + r) == c.name;
                }
                if (!l.visible) cp.visible = false;
                p.columns.append(cp);
            }
        }
        return;
    }
    // Container types with no children produce nothing to draw.
    if (l.type == "GroupLayer" || l.type == "BoneLayer" || l.type == "SwitchLayer")
        return;
    ColumnPlan c;
    c.name = path;
    c.type = l.type;
    c.imagePath = !l.imagePath.isEmpty() ? l.imagePath : l.texturePath;
    c.parentBone = bone;
    c.visible = l.visible;
    c.rows.fill(true, p.rowCount);
    p.columns.append(c);
}
}  // namespace detail

inline Plan makePlan(const Document &doc) {
    Plan p;
    p.firstFrame = doc.startFrame > 0 ? doc.startFrame : 1;
    p.rowCount = qMax(1, doc.endFrame - p.firstFrame + 1);
    for (const Layer &l : doc.layers)
        detail::flatten(doc, l, QString(), nullptr, -2, p);
    return p;
}

}  // namespace Moho
