#include "plugin_param_view_interface.h"

static int add_component(flare_param_view_handle_t view, void **handle,
                         std::shared_ptr<ParamView::Component> component) {
  if (ParamView *_ = reinterpret_cast<ParamView *>(view)) {
    if (handle) {
      *handle = component.get();
    }
    _->add_component(std::move(component));
  } else {
    return flare_ERROR_INVALID_HANDLE;
  }

  return flare_OK;
}

#define flare_DEFINE_ADD_COMPONENT(NAME, HANDLE, FIELD)                        \
  int NAME(flare_param_view_handle_t view, HANDLE *handle) {                   \
    return add_component(view, handle, std::make_shared<ParamView::FIELD>());  \
  }

flare_DEFINE_ADD_COMPONENT(add_param_field, flare_param_field_handle_t,
                           ParamField);
flare_DEFINE_ADD_COMPONENT(add_lineedit, flare_lineedit_handle_t, LineEdit);
flare_DEFINE_ADD_COMPONENT(add_slider, flare_slider_handle_t, Slider);
flare_DEFINE_ADD_COMPONENT(add_spinbox, flare_spinbox_handle_t, SpinBox);
flare_DEFINE_ADD_COMPONENT(add_checkbox, flare_checkbox_handle_t, CheckBox);
flare_DEFINE_ADD_COMPONENT(add_radiobutton, flare_radiobutton_handle_t,
                           RadioButton);
flare_DEFINE_ADD_COMPONENT(add_combobox, flare_combobox_handle_t, ComboBox);

#undef flare_DEFINE_ADD_COMPONENT

