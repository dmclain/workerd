// Copyright (c) 2026 Cloudflare, Inc.
// Licensed under the Apache 2.0 license found in the LICENSE file or at:
//     https://opensource.org/licenses/Apache-2.0

#include "jsg-test.h"

namespace workerd::jsg::test {
namespace {

V8System v8System;

// Stands in for a template built outside the type wrapper, such as a Rust resource's.
v8::Local<v8::FunctionTemplate> getWidgetTemplate(v8::Isolate* isolate) {
  auto tmpl = v8::FunctionTemplate::New(isolate);
  tmpl->SetClassName(v8StrIntern(isolate, "Widget"));
  tmpl->PrototypeTemplate()->Set(isolate, "kind", v8StrIntern(isolate, "widget"));
  return tmpl;
}

struct NestedTemplateContext: public Object, public ContextGlobal {
  JSG_RESOURCE_TYPE(NestedTemplateContext) {
    JSG_NESTED_TEMPLATE(Widget, getWidgetTemplate);
  }
};
JSG_DECLARE_ISOLATE_TYPE(NestedTemplateIsolate, NestedTemplateContext);

KJ_TEST("JSG_NESTED_TEMPLATE exposes a template built outside the type wrapper") {
  Evaluator<NestedTemplateContext, NestedTemplateIsolate> e(v8System);
  e.expectEval("typeof Widget", "string", "function");
  e.expectEval("Widget.name", "string", "Widget");
  e.expectEval("new Widget() instanceof Widget", "boolean", "true");
  e.expectEval("new Widget().kind", "string", "widget");
}

}  // namespace
}  // namespace workerd::jsg::test
