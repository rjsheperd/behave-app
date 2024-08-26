Feature: Surface and Crown Worksheets

  # BHP1-988
  Scenario: Fire Size > Fire Perimeter Output Selected
    Given I have started a Surface and Crown Worksheet
    When I select these outputs Submodule > Group > Output:
      """
      - Size > Fire Perimeter > Fire Perimeter
      """
    Then the following input Submodule > Groups are displayed:
      """
      - Fuel Model > Standard > Fuel Model
      - Fuel Moisture > Moisture Input Mode
      - Wind and Slope > Wind Speed
      - Wind and Slope > Wind and slope are
      - Wind and Slope > Slope
      """

  # BHP1-988
  Scenario: Fire Size > Fire Area Output Selected
    Given I have started a Surface and Crown Worksheet
    When I select these outputs Submodule > Group > Output:
      """
      - Size > Fire Area > Fire Area
      """
    Then the following input Submodule > Groups are displayed:
      """
      - Fuel Model > Standard > Fuel Model
      - Fuel Moisture > Moisture Input Mode
      - Wind and Slope > Wind Speed
      - Wind and Slope > Wind and slope are
      - Wind and Slope > Slope
      """

  # BHP1-988
  Scenario: Fire Size > Spread Distance Output Selected
    Given I have started a Surface and Crown Worksheet
    When I select these outputs Submodule > Group > Output:
      """
      - Size > Spread Distance > Spread Distance
      """
    Then the following input Submodule > Groups are displayed:
      """
      - Fuel Model > Standard > Fuel Model
      - Fuel Moisture > Moisture Input Mode
      - Wind and Slope > Wind Speed
      - Wind and Slope > Wind and slope are
      - Wind and Slope > Slope
      """
      
  # BHP1-979
  Scenario: Fire Type > Active Ratio Output Selected
    Given I have started a Surface and Crown Worksheet
    When I select these outputs Submodule > Group > Output:
      """
      - Fire Type > Active Crown Fire > Active Ratio
      """
    Then the following input Submodule > Groups are displayed:
      """
      - Fuel Model > Standard > Fuel Model
      - Fuel Moisture > Moisture Input Mode
      - Wind and Slope > Wind Speed
      - Wind and Slope > Wind and slope are
      - Wind and Slope > Slope
      """

  # BHP1-979
  Scenario: Fire Type > Critical Crown Rate of Spread Selected
    Given I have started a Surface and Crown Worksheet
    When I select these outputs Submodule > Group > Output:
      """
      - Fire Type > Active Crown Fire > Critical Crown Rate of Spread
      """
    Then the following input Submodule > Groups are displayed:
      """
      - Fuel Model > Standard > Fuel Model
      - Fuel Moisture > Moisture Input Mode
      - Wind and Slope > Wind Speed
      - Wind and Slope > Wind and slope are
      - Wind and Slope > Slope
      """

  # BHP1-979
  Scenario: Fire Type > Transition Ratio Selected
    Given I have started a Surface and Crown Worksheet
    When I select these outputs Submodule > Group > Output:
      """
      - Fire Type > Transition to Crown Fire > Transition Ratio
      """
    Then the following input Submodule > Groups are displayed:
      """
      - Fuel Model > Standard > Fuel Model
      - Fuel Moisture > Moisture Input Mode
      - Wind and Slope > Wind Speed
      - Wind and Slope > Wind and slope are
      - Wind and Slope > Slope
      """

  # BHP1-979
  Scenario: Fire Type > Critical Surface Flame Length Selected
    Given I have started a Surface and Crown Worksheet
    When I select these outputs Submodule > Group > Output:
      """
      - Fire Type > Transition to Crown Fire > Critical Surface Flame Length
      """
    Then the following input Submodule > Groups are displayed:
      """
      - Fuel Model > Standard > Fuel Model
      - Fuel Moisture > Moisture Input Mode
      - Wind and Slope > Wind Speed
      - Wind and Slope > Wind and slope are
      - Wind and Slope > Slope
      """

  # BHP1-979
  Scenario: Fire Type > Critical Surface Fireline Intensity Selected
    Given I have started a Surface and Crown Worksheet
    When I select these outputs Submodule > Group > Output:
      """
      - Fire Type > Transition to Crown Fire > Critical Surface Fireline Intensity
      """
    Then the following input Submodule > Groups are displayed:
      """
      - Fuel Model > Standard > Fuel Model
      - Fuel Moisture > Moisture Input Mode
      - Wind and Slope > Wind Speed
      - Wind and Slope > Wind and slope are
      - Wind and Slope > Slope
      """
