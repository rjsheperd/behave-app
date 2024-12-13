#include <functional>
#include "converter.cpp"
#include "surface.h"
#include "fuelModels.h"






class SIGTestClass {
public:
  void setHeight(double height, LengthUnits::LengthUnitsEnum units) {
    height_ = LengthUnits::toBaseUnits(height, units);
  };

  double getHeight(LengthUnits::LengthUnitsEnum units) {
    return LengthUnits::fromBaseUnits(height_, units);
  };

  void setSpeed(double speed, SpeedUnits::SpeedUnitsEnum units) {
    speed_ = SpeedUnits::toBaseUnits(speed, units);
  };

  double getSpeed(SpeedUnits::SpeedUnitsEnum units) {
    return SpeedUnits::fromBaseUnits(speed_, units);
  };

  void setUnits(LengthUnits::LengthUnitsEnum units) {
    units_ = units;
  }

  LengthUnits::LengthUnitsEnum getUnits() {
    return units_;
  }

private:
  double height_;
  double speed_;
  LengthUnits::LengthUnitsEnum units_;
};

int main() {
  std::unordered_map<std::string, std::function<void(void*, std::string)>> discrete_setters;
  std::unordered_map<std::string, std::function<void(void*, double, std::string)>> cont_setters;
  std::unordered_map<std::string, std::function<double(void*, std::string)>> getters;

  UnitResolver unitResolver;

  FuncConverter fnConverter(unitResolver);

  SIGTestClass sut;

  // sut.setSpeed(30.0, SpeedUnits::MilesPerHour);
  cont_setters["setHeight"] = fnConverter.convertContinuousSetter(&SIGTestClass::setHeight);
  cont_setters["setHeight"](&sut, 10.0, "mi");

  cont_setters["setSpeed"] = fnConverter.convertContinuousSetter(&SIGTestClass::setSpeed);
  cont_setters["setSpeed"](&sut, 30.0, "mi/h");

  discrete_setters["setUnits"] = fnConverter.convertDiscreteSetter(&SIGTestClass::setUnits);
  discrete_setters["setUnits"](&sut, "3");

  getters["getSpeed"] = fnConverter.convertGetter(&SIGTestClass::getSpeed);
  std::cout << getters["getSpeed"](&sut, "ft/min") << std::endl;
  std::cout << sut.getSpeed(SpeedUnits::FeetPerMinute) << std::endl;

  getters["getHeight"] = fnConverter.convertGetter(&SIGTestClass::getHeight);
  std::cout << getters["getHeight"](&sut, "ft") << std::endl;
  std::cout << sut.getHeight(LengthUnits::Feet) << std::endl;

  std::cout << sut.getUnits() << std::endl;

  return 0;
}
