(ns behave.schema.queries
  (:require [clojure.string :as str]
            #?(:cljs [datascript.core :as d]
               :clj  [datahike.api :as d])))

;;; Rules

(def rules
  '[
    ;; -- Find an entity's UUID
    [(uuid ?e ?uuid)
     [?e :bp/uuid ?uuid]]

    ;; -- Find an entity's name
    [(name ?e ?name)
     [?e :application/name ?name]]

    [(name ?e ?name)
     [?e :module/name ?name]]

    [(name ?e ?name)
     [?e :submodule/name ?name]]

    [(name ?e ?name)
     [?e :group/name ?name]]

    [(name ?e ?name)
     [?e :group-variable/name ?name]]

    ;; --  Recursive rules to find a group's subgroups 
    [(subgroup ?g ?s)
     [?g :group/children ?s]]

    [(subgroup ?g ?s)
     [?g :group/children ?x]
     (subgroup ?x ?s)]

    ;; --  Recursive rule to find a submodule's groups
    [(group ?s ?g)
     [?s :submodule/groups ?g]]

    [(group ?s ?g)
     [?s :submodule/groups ?x]
     (subgroup ?x ?g)]

    ;; --  Submodule of a module
    [(submodule ?m ?s)
     [?m :module/submodules ?s]]

    ;; --  Application's modules
    [(module ?a ?m)
     [?e :application/modules ?m]]

    ;; --  Group's group-variables
    [(variable ?g ?gv ?v)
     [?g :group/group-variables ?gv]
     [?v :variable/group-variables ?gv]]

    ;; -- Entity's Input/Ouput (Group Variable, Group, Submodule)
    [(io ?e ?io)
     [?e :submodule/io ?io]]
    
    [(io ?e ?io)
     (group ?s ?e)
     [?e :submodule/io ?io]]

    [(io ?e ?io)
     (variable ?g ?e)
     (group ?s ?g)
     [?e :submodule/io ?io]]

    ;; --  Find the root application for a module, submodule, group, or subgroup
    [(app-root ?a ?g)
     [?sm :submodule/groups ?g]
     [?m :module/submodules ?sm]
     [?a :application/modules ?m]]

    [(app-root ?a ?s)
     (subgroup ?g ?s)
     [?sm :submodule/groups ?g]
     [?m :module/submodules ?sm]
     [?a :application/modules ?m]]

    ;; --  Language of an
    [(language ?code ?l)
     [?l :language/shortcode ?code]]

    ;; --  Entity's translation key
    [(translation-key ?e ?k)
     [?e :application/translation-key ?k]]

    [(translation-key ?e ?k)
      [?e :module/translation-key ?k]]

    [(translation-key ?e ?k)
      [?e :submodule/translation-key ?k]]

    [(translation-key ?e ?k)
      [?e :group/translation-key ?k]]

    [(translation-key ?e ?k)
      [?e :group-variable/translation-key ?k]]

    ;; --  Translation key to translation
    [(translation ?k ?t ?content)
     [?t :translation/key ?k]
     [?t :translation/translation ?content]]

    ;; --  Entity's help key
    [(translation-key ?e ?k)
      [?e :application/help-key ?k]]

    [(translation-key ?e ?k)
      [?e :module/help-key ?k]]

    [(translation-key ?e ?k)
      [?e :submodule/help-key ?k]]

    [(translation-key ?e ?k)
      [?e :group/help-key ?k]]

    [(translation-key ?e ?k)
      [?e :group-variable/help-key ?k]]

    ;; -- CPP Relations
    [(cpp-enum ?n ?e)
     [?e :cpp.namespace/enum ?e]]

    [(cpp-enum-member ?e ?m ?v)
     [?e :cpp.enum/enum-member ?m]
     [?e :cpp.enum-member/value ?v]]

    [(cpp-class ?n ?c)
     [?e :cpp.namespace/class ?c]]

    [(cpp-fn ?c ?f)
     [?c :cpp.class/function ?f]]

    [(cpp-param ?f ?p)
     [?f :cpp.function/parameter ?p]]

    ;; -- CPP Names
    [(cpp-name ?e ?name)
     [?e :cpp.namespace/name ?name]]

    [(cpp-name ?e ?name)
     [?e :cpp.class/name ?name]]

    [(cpp-name ?e ?name)
     [?e :cpp.function/name ?name]]

    [(cpp-name ?e ?name)
     [?e :cpp.parameter/name ?name]]

    ;; -- Lookup another entity by a shared UUID
    [(ref ?uuid1 ?attr ?e2)
     (uuid ?e1 ?uuid1)
     [?e1 ?rel ?uuid2]
     (uuid ?e2 ?uuid2)]

    ;; -- Find a group variable's variable
    [(gv->var ?uuid ?v)
     (uuid ?gv ?uuid)
     [?v :variable/group-variables ?gv]]

    ;; -- Find a variable's units
    [(var-units ?uuid ?units)
     (gv->var ?uuid ?v)
     [?v :variable/native-units ?units]]

    ;; -- Find a variable's kind
    [(kind ?uuid ?kind)
     (gv->var ?uuid ?v)
     [?v :variable/kind ?kind]]

    ;; -- Find a group variable's function
    [(var->fn ?uuid ?fn)
     (ref ?uuid :group-variable/cpp-function ?fn)]

    ;; -- Find a group variable's parameter
    [(var->param ?uuid ?p)
     (ref ?uuid :group-variable/cpp-parameter ?p)]

    [(param-attrs ?p ?p-name ?p-type ?p-order)
     [?p cpp.parameter/name ?p-name]
     [?p cpp.parameter/type ?p-type]
     [?p cpp.parameter/order ?p-order]]

    ;; Find the function's parameters
    [(fn-params ?fn ?p ?p-name ?p-type ?p-order)
     [?fn :cpp.function/parameters ?p]
     (param-attrs ?p ?p-name ?p-type ?p-order)]

    [(subgroup ?g ?sg) [?g :group/children ?sg]]

    [(module-output-vars ?m ?gv)
     [?m :module/submodules ?s]
     [?s :submodule/io :output]
     [?s :submodule/groups ?g]
     [?g :group/group-variables ?gv]]

    ;; -- Find all output functions for a given module
    [(module-output-fns ?m ?fn ?fn-name)
     (module-output-vars ?m ?gv)
     (lookup ?uuid ?gv)
     (var->fn ?uuid ?fn)
     [?fn :cpp.function/name ?fn-name]]

    ;; -- Find all input group variables for a given module
    [(module-input-vars ?m ?gv)
     (submodule ?m ?s)
     (io ?s :input)
     (group ?s ?g)
     (variable ?g ?gv)]

    [(module-input-fns ?m ?fn ?fn-name)
     (module-input-vars ?m ?gv)
     (lookup ?gv ?uuid)
     (var->fn ?uuid ?fn)
     (cpp-name ?fn ?fn-name)]])

;;; Public Fns

(defn q [query conn & args]
  (apply d/q query conn rules args))

(defn pull-children [conn child-attr eid & [pattern]]
  (d/pull-many conn
               (or pattern '[*])
               (q '[:find [?c ...]
                    :in $ % ?child-attr ?e
                    :where [?e ?child-attr ?c]]
                  conn child-attr eid)))

(defn pull-with-attr [conn attr & [pattern]]
  (d/pull-many conn
               (or pattern '[*])
               (q '[:find [?e ...]
                    :in $ % ?attr
                    :where [?e ?attr]]
                  conn attr)))
