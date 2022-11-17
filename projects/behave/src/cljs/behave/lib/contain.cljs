(ns behave.lib.contain)

;; Initializer
(defn init []
  (js/Module.SIGContainAdapter.))

;; Run function
(defn doContainRun [self]
  (.doContainRun self)
  self)

; Inputs
(defn addResource [self arrival duration timeUnit productionRate productionRateUnits description]
  (.addResource self arrival duration timeUnit productionRate productionRateUnits description)
  self)

(defn setAttackDistance [self attackDistance lengthUnits]
  (.setAttackDistance self attackDistance lengthUnits)
  self)

(defn setFireStartTime [self fireStartTime]
  (.setFireStartTime self fireStartTime)
  self)

(defn setLwRatio [self lwRatio]
  (.setLwRatio self lwRatio)
  self)

(defn setMaxFireSize [self maxFireSize]
  (.setMaxFireSize self maxFireSize)
  self)

(defn setMaxFireTime [self maxFireTime]
  (.setMaxFireTime self maxFireTime)
  self)

(defn setMaxSteps [self maxSteps]
  (.setMaxSteps self maxSteps)
  self)

(defn setMinSteps [self minSteps]
  (.setMinSteps self minSteps)
  self)

(defn setReportRate [self reportRate speedUnits]
  (.setReportRate self reportRate speedUnits)
  self)

(defn setReportSize [self reportSize areaUnits]
  (.setReportSize self reportSize areaUnits)
  self)

(defn setRetry [self retry]
  (.setRetry self retry)
  self)

(defn setTactic [self tactic]
  (.setTactic self tactic)
  self)

(defn removeAllResources [self]
  (.removeAllResources self)
  self)

(defn removeResourceWithThisDesc [self desc]
  (.removeResourceWithThisDesc self desc)
  self)

(defn removeResourceAt [self index]
  (.removeResourceAt self index)
  self)

(defn removeAllResourcesWithThisDesc [self desc]
  (.removeAllResourcesWithThisDesc self desc)
  self)

; Outputs
(defn getContainmentStatus [self]
  (.getContainmentStatus self))

(defn getFinalContainmentArea [self areaUnits]
  (.getFinalContainmentArea self areaUnits))

(defn getFinalCost [self]
  (.getFinalCost self))

(defn getFinalFireLineLength [self lengthUnits]
  (.getFinalFireLineLength self lengthUnits))

(defn getFinalFireSize [self areaUnits]
  (.getFinalFireSize self areaUnits))

(defn getFinalTimeSinceReport [self timeUnits]
  (.getFinalTimeSinceReport self timeUnits))

(defn getFireSizeAtInitialAttack [self areaUnits]
  (.getFireSizeAtInitialAttack self areaUnits))

(defn getPerimeterAtContainment [self lengthUnits]
  (.getPerimeterAtContainment self lengthUnits))

(defn getPerimeterAtInitialAttack [self lengthUnits]
  (.getPerimeterAtInitialAttack self lengthUnits))
