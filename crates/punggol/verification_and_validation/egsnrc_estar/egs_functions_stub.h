#pragma once
#include <cstdio>
#include <cstdlib>
#include <cstdarg>
#include <string>
#include <cerrno>
#include <cctype>
#include <iostream>
using namespace std;
inline void egsFatal(const char* f, ...) { va_list a; va_start(a, f); vfprintf(stderr, f, a); va_end(a); exit(3); }
inline void egsInformation(const char*, ...) {}
