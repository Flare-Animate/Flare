#pragma once
#include <string>
struct TFilePath { std::wstring w; std::wstring getWideString() const { return w; } };
inline std::string toString(const std::wstring &s) { return std::string(s.begin(), s.end()); }
class CasmFileInfo { public: CasmFileInfo(const TFilePath &fp); void getFrameRange(int &, int &, bool &); private: TFilePath m_fp; };
