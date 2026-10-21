//------------------------------------------------------------------------------
/*! \file SIGSoilHeating.h
    \brief behave-lib adapter for SoilHeating (FOFEM soil heating): the inputs' setters and
    getters as methods of the module, so the app can call them on one object,
    and the message as char* for WebIDL.
*/
#pragma once
#include "soilHeating.h"
#include "SIGString.h"

class SIGSoilHeating : public SoilHeating {
public:
  SIGSoilHeating();
  SIGSoilHeating(const SIGSoilHeating& rhs);
  SIGSoilHeating& operator=(const SIGSoilHeating& rhs);

  void initializeMembers();

  /*! \brief The last run's message (SoilHeating::getMessage), as char*. */
  char* getMessage() const;

  // Inputs: forwarded to SoilHeatingInputs
  void setSoilFamily(SoilFamily::SoilFamilyEnum soilFamily);
  void setSoilMoisture(double soilMoisture, FractionUnits::FractionUnitsEnum moistureUnits);
  void setStartingSoilTemperature(double temperature, TemperatureUnits::TemperatureUnitsEnum temperatureUnits);
  void setDuffDepthPrefire(double duffDepth, LengthUnits::LengthUnitsEnum lengthUnits);
  void setDuffDepthPostfire(double duffDepth, LengthUnits::LengthUnitsEnum lengthUnits);
  void setDuffLoadPrefire(double duffLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setDuffMoisture(double duffMoisture, FractionUnits::FractionUnitsEnum moistureUnits);
  void setDuffConsumed(double duffConsumed, FractionUnits::FractionUnitsEnum fractionUnits);
  void setDuffHeatEfficiency(double efficiency, FractionUnits::FractionUnitsEnum fractionUnits);
  void setWoodLitterHeatEfficiency(double efficiency, FractionUnits::FractionUnitsEnum fractionUnits);
  void setHerbShrubHeatEfficiency(double efficiency, FractionUnits::FractionUnitsEnum fractionUnits);
  void setFuelbedIgnited(bool isFuelbedIgnited);
  void clearFireIntensities();
  void addFireIntensity(double woodLitterIntensity, double herbShrubIntensity, HeatSourceAndReactionIntensityUnits::HeatSourceAndReactionIntensityUnitsEnum intensityUnits);
  SoilFamily::SoilFamilyEnum getSoilFamily() const;
  double getSoilMoisture(FractionUnits::FractionUnitsEnum moistureUnits) const;
  double getStartingSoilTemperature(TemperatureUnits::TemperatureUnitsEnum temperatureUnits) const;
  double getDuffDepthPrefire(LengthUnits::LengthUnitsEnum lengthUnits) const;
  double getDuffDepthPostfire(LengthUnits::LengthUnitsEnum lengthUnits) const;
  double getDuffLoadPrefire(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getDuffMoisture(FractionUnits::FractionUnitsEnum moistureUnits) const;
  double getDuffConsumed(FractionUnits::FractionUnitsEnum fractionUnits) const;
  double getDuffHeatEfficiency(FractionUnits::FractionUnitsEnum fractionUnits) const;
  double getWoodLitterHeatEfficiency(FractionUnits::FractionUnitsEnum fractionUnits) const;
  double getHerbShrubHeatEfficiency(FractionUnits::FractionUnitsEnum fractionUnits) const;
  bool getFuelbedIgnited() const;
  int getNumberOfFireIntensities() const;
  double getWoodLitterFireIntensity(int index, HeatSourceAndReactionIntensityUnits::HeatSourceAndReactionIntensityUnitsEnum intensityUnits) const;
  double getHerbShrubFireIntensity(int index, HeatSourceAndReactionIntensityUnits::HeatSourceAndReactionIntensityUnitsEnum intensityUnits) const;
};
