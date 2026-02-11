#pragma once

#ifndef PLUGIN_PARAM_VIEW_INTERFACE
#define PLUGIN_PARAM_VIEW_INTERFACE

#include "tfx.h"
#include "flare_hostif.h"

#include <flareqt/fxsettings.h>

#include <QWidget>

#include <vector>
#include <memory>

/* 公開ヘッダからは引っ込められた都合上ここで宣言(内部ではまだ使っているので) */
typedef void *flare_param_view_handle_t;

/* あるパラメータの表示形式 */
class ParamView {
public:
  struct Component {
    virtual ~Component() {}
    virtual QWidget *make_widget(TFx *fx, ParamsPage *page,
                                 const char *name) const = 0;
  };

#define flare_DEFINE_COMPONENT(NAME)                                           \
  struct NAME : public Component {                                             \
    QWidget *make_widget(TFx *fx, ParamsPage *page,                            \
                         const char *name) const final override {              \
      return page->new##NAME(fx, name);                                        \
    }                                                                          \
  }

  flare_DEFINE_COMPONENT(ParamField);
  flare_DEFINE_COMPONENT(LineEdit);
  flare_DEFINE_COMPONENT(Slider);
  flare_DEFINE_COMPONENT(SpinBox);
  flare_DEFINE_COMPONENT(CheckBox);
  flare_DEFINE_COMPONENT(RadioButton);
  flare_DEFINE_COMPONENT(ComboBox);

#undef flare_DEFINE_COMPONENT

public:
  ParamView() {}

  ParamView(const ParamView &rhs) : components_(rhs.components_) {}

  inline void add_component(std::shared_ptr<Component> component) {
    components_.push_back(std::move(component));
  }

  ParamView *clone(TFx *) const { return new ParamView(*this); }

  void build(TFx *fx, ParamsPage *page, const char *name) const {
    for (const auto &component : components_) {
      page->addWidget(component->make_widget(fx, page, name));
    }
  }

private:
  std::vector<std::shared_ptr<Component>> components_;
};

typedef void *flare_param_field_handle_t;
typedef void *flare_lineedit_handle_t;
typedef void *flare_slider_handle_t;
typedef void *flare_spinbox_handle_t;
typedef void *flare_checkbox_handle_t;
typedef void *flare_radiobutton_handle_t;
typedef void *flare_combobox_handle_t;

int add_param_field(flare_param_view_handle_t view,
                    flare_param_field_handle_t *ParamField);
int add_lineedit(flare_param_view_handle_t view,
                 flare_lineedit_handle_t *LineEdit);
int add_slider(flare_param_view_handle_t view, flare_slider_handle_t *Slider);
int add_spinbox(flare_param_view_handle_t view,
                flare_spinbox_handle_t *CheckBox);
int add_checkbox(flare_param_view_handle_t view,
                 flare_checkbox_handle_t *CheckBox);
int add_radiobutton(flare_param_view_handle_t view,
                    flare_radiobutton_handle_t *RadioButton);
int add_combobox(flare_param_view_handle_t view,
                 flare_combobox_handle_t *Combobox);

#undef flare_DEFINE_ADD_COMPONENT

#endif

