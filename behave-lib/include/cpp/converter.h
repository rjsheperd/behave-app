#include <functional>
#include "unitResolver.h"

#pragma once

class FuncConverter {
private:
  UnitResolver _unitResolver;
public:
  FuncConverter(UnitResolver &unitResolver) : _unitResolver(unitResolver) {}

  // Discrete Setter
  template<typename ObjType, typename EnumType>
  void wrapperDiscreteSetter(ObjType* obj, void (ObjType::*func)(EnumType), std::string str) {
    // Convert the input string to the desired EnumType
    EnumType enumValue = static_cast<EnumType>(std::stoi(str));

    // Call the member function of the object with the converted EnumType
    (obj->*func)(enumValue);
  }

  template<typename ObjType, typename EnumType>
  std::function<void(void*, std::string)> convertDiscreteSetter(void (ObjType::*func)(EnumType)) {
    return [=](void* obj, std::string str) {
      wrapperDiscreteSetter(static_cast<ObjType*>(obj), func, str);
    };
  }

  // Continous Setter
  template<typename ObjType, typename EnumType>
  void wrapperContinuousSetter(ObjType* obj, void (ObjType::*func)(double, EnumType), double num, std::string str) {
    // Convert the input string to the desired EnumType
    EnumType enumValue = _unitResolver.resolveUnit<EnumType>(str);

    // Call the member function of the object with the converted EnumType
    (obj->*func)(num, enumValue);
  }

  template<typename ObjType, typename EnumType>
  std::function<void(void*, double, std::string)> convertContinuousSetter(void (ObjType::*func)(double, EnumType)) {
    return [=](void* obj, double num, std::string str) {
      wrapperContinuousSetter(static_cast<ObjType*>(obj), func, num, str);
    };
  }

  // Calculate Wrapper
  template<typename ObjType>
  std::function<void(void*)> convertCalculate(void (ObjType::*func)()) {
    return [=](void* obj, std::string str) {
      (obj->*func)();
    };
  }

  // Getters
  template<typename ObjType, typename EnumType>
  double wrapperGetter(ObjType* obj, double (ObjType::*func)(EnumType), std::string str) {
    // Convert the input string to the desired EnumType
    EnumType enumValue = _unitResolver.resolveUnit<EnumType>(str);

    // Call the member function of the object with the converted EnumType
    return (obj->*func)(enumValue);
  }

  template<typename ObjType, typename EnumType>
  std::function<double(void*, std::string)> convertGetter(double (ObjType::*func)(EnumType)) {
    return [=](void* obj, std::string str) {
      return wrapperGetter(static_cast<ObjType*>(obj), func, str);
    };
  }
};
