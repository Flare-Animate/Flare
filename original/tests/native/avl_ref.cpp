// Reference driver for gen_avl_cases.py: links the ORIGINAL avl.c (built from git
// with gcc) and replays a script, one result per line ("<line> => <result>").
//   T <kind>                       new tree (kind: nodup_mbr ... dup_char); frees the previous one
//   M id i s us ui ul l c str m    define item id (the item table is cleared by T)
//   I id                           avl_insert           -> 0|1
//   L eq|ge|gt|le|lt key           avl__locate*         -> id|-1
//   F / Z                          avl_locate_first/last -> id|-1
//   R key                          avl__remove          -> id|-1
//   S / B                          scan / backscan      -> ids
//   W key back                     avl__start, then next/prev until NULL (max 60) -> ids
//   G back                         avl_first/last then next/prev (max 60) -> ids
//   K back                         avl__link / backlink chain -> ids
//   D                              avl_dump -> depth then id:lev:pos ...
//   C                              avl_copy, scan the copy, avl_close it -> ids
//   N                              node count
//   E                              avl_release (callback order) -> ids   (tree stays usable)
//   Q                              avl_reset
//   P ncol npen hexstring          cdb_decode_all -> one record per tree item
// Keys: integers, or a bare word for the string kinds; for mbr/ptr kinds the integer is
// the value of an int the key pointer points at.
#include <csetjmp>
#include <csignal>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <sstream>
extern "C" {
#include "avl.h"
#include "tcm.h"
}

struct Item {
  int id;
  int i;
  short s;
  unsigned short us;
  unsigned ui;
  unsigned long ul;
  long l;
  char c;
  char strbuf[12];
  char *str;
  int m;
  int cell;
  void *pk;
  void *next;
};

static Item items[64];
static bool defined[64];
static std::vector<int> collected;
static int cmpint(void *a, void *b) { return *(int *)a - *(int *)b; }
static void collect(void *p) { collected.push_back(((Item *)p)->id); }
static int ID(void *p) { return p ? ((Item *)p)->id : -1; }

struct Kind { const char *name; int type; unsigned off; bool user; bool str; };
#define OFF(f) ((unsigned)offsetof(Item, f))
static const Kind kinds[] = {
    {"nodup_mbr", AVL_NODUP_MBR, OFF(m), true, false}, {"dup_mbr", AVL_DUP_MBR, OFF(m), true, false},
    {"nodup_ptr", AVL_NODUP_PTR, OFF(pk), true, false}, {"dup_ptr", AVL_DUP_PTR, OFF(pk), true, false},
    {"nodup_str", AVL_NODUP_STR, OFF(str), false, true}, {"dup_str", AVL_DUP_STR, OFF(str), false, true},
    {"nodup_long", AVL_NODUP_LNG, OFF(l), false, false}, {"dup_long", AVL_DUP_LNG, OFF(l), false, false},
    {"nodup_int", AVL_NODUP_INT, OFF(i), false, false}, {"dup_int", AVL_DUP_INT, OFF(i), false, false},
    {"nodup_short", AVL_NODUP_SHT, OFF(s), false, false}, {"dup_short", AVL_DUP_SHT, OFF(s), false, false},
    {"nodup_ulong", AVL_NODUP_ULN, OFF(ul), false, false}, {"dup_ulong", AVL_DUP_ULN, OFF(ul), false, false},
    {"nodup_uint", AVL_NODUP_UIN, OFF(ui), false, false}, {"dup_uint", AVL_DUP_UIN, OFF(ui), false, false},
    {"nodup_ushort", AVL_NODUP_USH, OFF(us), false, false}, {"dup_ushort", AVL_DUP_USH, OFF(us), false, false},
    {"nodup_char", AVL_NODUP_CHR, OFF(c), false, false}, {"dup_char", AVL_DUP_CHR, OFF(c), false, false},
};

static std::string ids(const std::vector<int> &v) {
  if (v.empty()) return "-";
  std::string s;
  for (size_t k = 0; k < v.size(); k++) { if (k) s += ","; s += std::to_string(v[k]); }
  return s;
}
static std::string unhex(const std::string &h) {
  std::string s;
  for (size_t k = 0; k + 1 < h.size(); k += 2) s += (char)std::stoi(h.substr(k, 2), 0, 16);
  return s;
}
static std::string cs(const char *p) { return p ? std::string(p) : std::string("<null>"); }

static sigjmp_buf jb;
static void onsegv(int) { siglongjmp(jb, 1); }

int main() {
  signal(SIGSEGV, onsegv);
  TREE *tree = 0;
  const Kind *kind = 0;
  static int probe;
  char buf[1 << 16];
  while (fgets(buf, sizeof buf, stdin)) {
    std::string line(buf);
    while (!line.empty() && (line.back() == '\n' || line.back() == '\r')) line.pop_back();
    if (line.empty()) continue;
    std::istringstream in(line);
    std::string op;
    in >> op;
    std::string res = "-";
    auto key = [&](const std::string &tok) -> long {
      static std::string keep;
      if (kind->str) { keep = tok; return (long)keep.c_str(); }
      long v = strtol(tok.c_str(), 0, 10);
      if (kind->user) { probe = (int)v; return (long)&probe; }
      return v;
    };
    if (op == "T") {
      std::string k; in >> k;
      if (tree) avl_close(tree);
      for (auto &d : defined) d = false;
      kind = 0;
      for (auto &x : kinds) if (k == x.name) kind = &x;
      tree = avl__tree(kind->type, kind->off, kind->user ? cmpint : 0);
    } else if (op == "M") {
      int id; in >> id;
      Item &it = items[id];
      memset(&it, 0, sizeof it);
      std::string str;
      long long ul, l; int i, s, us, c, m; unsigned long long ui;
      in >> i >> s >> us >> ui >> ul >> l >> c >> str >> m;
      it.id = id; it.i = i; it.s = (short)s; it.us = (unsigned short)us; it.ui = (unsigned)ui;
      it.ul = (unsigned long)ul; it.l = (long)l; it.c = (char)c; strncpy(it.strbuf, str.c_str(), 11); it.str = it.strbuf;
      it.m = m; it.cell = m; it.pk = &it.cell;
      defined[id] = true;
    } else if (op == "I") {
      int id; in >> id;
      res = std::to_string(avl_insert(tree, &items[id]));
    } else if (op == "L") {
      std::string rel, tok; in >> rel >> tok;
      long k = key(tok);
      void *r = rel == "eq" ? avl__locate(tree, k) : rel == "ge" ? avl__locate_ge(tree, k)
              : rel == "gt" ? avl__locate_gt(tree, k) : rel == "le" ? avl__locate_le(tree, k)
              : avl__locate_lt(tree, k);
      res = std::to_string(ID(r));
    } else if (op == "F") res = std::to_string(ID(avl_locate_first(tree)));
    else if (op == "Z") res = std::to_string(ID(avl_locate_last(tree)));
    else if (op == "R") {
      std::string tok; in >> tok;
      res = std::to_string(ID(avl__remove(tree, key(tok))));
    } else if (op == "S" || op == "B") {
      collected.clear();
      avl__scan(tree, collect, op == "B");
      res = ids(collected);
    } else if (op == "W") {
      std::string tok; int back; in >> tok >> back;
      std::vector<int> v;
      void *p = avl__start(tree, key(tok), back);
      while (p && v.size() < 60) { v.push_back(ID(p)); p = back ? avl_prev(tree) : avl_next(tree); }
      res = ids(v);
      avl_stop(tree);
    } else if (op == "G") {
      int back; in >> back;
      std::vector<int> v;
      void *p = back ? avl_last(tree) : avl_first(tree);
      while (p && v.size() < 60) { v.push_back(ID(p)); p = back ? avl_prev(tree) : avl_next(tree); }
      res = ids(v);
      avl_stop(tree);
    } else if (op == "K") {
      int back; in >> back;
      void *p = avl__link(tree, OFF(next), back);
      std::vector<int> v;
      while (p && v.size() < 80) { v.push_back(ID(p)); p = ((Item *)p)->next; }
      res = ids(v);
    } else if (op == "D") {
      void *dat[80]; int lev[80], pos[80];
      int depth = avl_dump(tree, dat, lev, pos);
      res = std::to_string(depth);
      for (long k = 0; k < tree->nodes; k++) res += " " + std::to_string(ID(dat[k])) + ":" + std::to_string(lev[k]) + ":" + std::to_string(pos[k]);
    } else if (op == "C") {
      TREE *c = avl_copy(tree);
      collected.clear();
      avl__scan(c, collect, 0);
      res = std::to_string(c->nodes) + " " + ids(collected);
      avl_close(c);
    } else if (op == "N") res = std::to_string(tree->nodes);
    else if (op == "E") {
      collected.clear();
      avl_release(tree, collect);
      res = ids(collected) + " " + std::to_string(tree->nodes);
    } else if (op == "Q") { avl_reset(tree); res = std::to_string(tree->nodes); }
    else if (op == "P") {
      int nc, np; std::string h; in >> nc >> np >> h;
      std::string s = unhex(h);
      std::vector<char> mem(s.begin(), s.end());
      mem.resize(s.size() + 256, 0);
      TCM_INFO tcm = Tcm_new_default_info;
      tcm.n_colors = (short)nc; tcm.n_pencils = (short)np;
      if (sigsetjmp(jb, 1)) {  // the original dereferenced garbage (UB): not recorded
        printf("%s => SKIP\n", line.c_str());
        fflush(stdout);
        continue;
      }
      TREE *t = cdb_decode_all(mem.data(), tcm);
      res = "";
      for (CDB_TREE_ITEM *p = (CDB_TREE_ITEM *)avl_first(t); p; p = (CDB_TREE_ITEM *)avl_next(t)) {
        res += std::to_string(p->index) + "|" + cs(p->group) + "|" + cs(p->name) + "|" + cs(p->accelerator) + "|" + std::to_string(p->num_effects) + "|";
        if (p->effects) {
          for (void *e = avl_first(p->effects); e; e = avl_next(p->effects)) {
            struct Param { char *name; char *value; };
            struct Eff { char *name; int num_params; Param *param; };
            Eff *ef = (Eff *)e;
            res += cs(ef->name) + ":" + std::to_string(ef->num_params) + "(";
            for (int k = 0; k < ef->num_params; k++) res += (k ? "," : "") + cs(ef->param[k].name) + "=" + cs(ef->param[k].value);
            res += ");";
          }
        }
        res += "#";
      }
      if (res.empty()) res = "-";
      // (UB cases crash and print SKIP.) the parse may print "error parsing color names and fx" on stdout; the generator
      // keeps only lines containing " => ".
    }
    printf("%s => %s\n", line.c_str(), res.c_str());
    fflush(stdout);
  }
  if (tree) avl_close(tree);
}
