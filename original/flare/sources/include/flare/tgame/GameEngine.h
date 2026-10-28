// GameEngine.h - Core game engine for Flare
// Copyright (c) 2026 Flare Project
//
// Provides a minimal game engine integrated with Flare's timeline and rendering,
// enabling Flash-style game creation. Designed to interoperate with Flare's
// existing scene, level, and asset systems.
//
// Architecture (kept deliberately small):
//   GameEngine         - singleton, owns the fixed-timestep loop
//   Scene              - owns entities and systems
//   Entity             - ID + component bitmask
//   Component          - pure data (no logic)
//   System             - logic operating on entities with matching components
//   AssetManager       - loads SWF, images, audio via Flare's existing IO
//
// Inspired by:
//   - Citrus Engine (StateMachine, Physics, Display)
//   - Simple Game Engine (ECS, Scene management)
//   - Ruffle (AVM2, DisplayList, Flash API)
//   - JPEXS (ABC parsing, SWF structure)

#ifndef TGAME_GAMEENGINE_H_
#define TGAME_GAMEENGINE_H_

#include "tcommon.h"
#include "tfilepath.h"
#include <QObject>
#include <QString>
#include <QMap>
#include <QVector>
#include <QSet>
#include <functional>
#include <memory>
#include <cstdint>

#undef DVAPI
#undef DVVAR
#ifdef TGAME_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

namespace TGame {

// Forward declarations
class Scene;
class Entity;
class Component;
class System;
class AssetManager;
class InputManager;
class PhysicsWorld;
class AudioManager;
class RenderSystem;

// Type aliases
using EntityID = uint32_t;
using ComponentID = uint32_t;
using SystemID = uint32_t;
using EntityMask = uint64_t;  // bitmask of components (supports up to 64)

// Invalid entity ID
constexpr EntityID INVALID_ENTITY = 0;

// Forward declare component types
struct TransformComponent;
struct SpriteComponent;
struct PhysicsBodyComponent;
struct ScriptComponent;
struct AudioComponent;
struct CameraComponent;
struct UIComponent;
struct ParticleComponent;

// Component registration
DVAPI ComponentID registerComponentType(const char* name);
DVAPI const char* getComponentName(ComponentID id);

//------------------------------------------------------------------
// Component - pure data, no logic
//------------------------------------------------------------------
struct DVAPI Component {
    EntityID entity = INVALID_ENTITY;
    ComponentID type = 0;
    bool active = true;

    virtual ~Component() = default;
    virtual Component* clone() const = 0;
};

//------------------------------------------------------------------
// TransformComponent - position, rotation, scale
//------------------------------------------------------------------
struct DVAPI TransformComponent : Component {
    double x = 0, y = 0;
    double rotation = 0;  // radians
    double scaleX = 1, scaleY = 1;
    double pivotX = 0.5, pivotY = 0.5;  // normalized pivot

    TransformComponent() { type = registerComponentType("Transform"); }
    Component* clone() const override;
};

//------------------------------------------------------------------
// SpriteComponent - renderable graphics (can reference Flare levels/SWF)
//------------------------------------------------------------------
struct DVAPI SpriteComponent : Component {
    TFilePath resourcePath;  // .swf, .png, .fla, etc.
    QString symbolName;      // for SWF symbols
    int frame = 0;
    bool visible = true;
    double alpha = 1.0;
    int renderOrder = 0;     // z-order within layer
    QString blendMode = "normal";  // normal, add, multiply, screen

    SpriteComponent() { type = registerComponentType("Sprite"); }
    Component* clone() const override;
};

//------------------------------------------------------------------
// PhysicsBodyComponent - Box2D integration
//------------------------------------------------------------------
struct DVAPI PhysicsBodyComponent : Component {
    enum BodyType { Static, Kinematic, Dynamic };
    BodyType type = Dynamic;
    double density = 1.0;
    double friction = 0.2;
    double restitution = 0.0;
    bool fixedRotation = false;
    bool sensor = false;
    QVector<QPair<double, double>> shapeVertices;  // polygon shape
    double radius = 0;  // for circle
    QString collisionGroup;  // collision filtering
    uint16_t categoryBits = 0x0001;
    uint16_t maskBits = 0xFFFF;

    // Runtime (not serialized)
    void* bodyPtr = nullptr;  // b2Body*

    PhysicsBodyComponent() { type = registerComponentType("PhysicsBody"); }
    Component* clone() const override;
};

//------------------------------------------------------------------
// ScriptComponent - AS3/Lua/JS script attachment
//------------------------------------------------------------------
struct DVAPI ScriptComponent : Component {
    TFilePath scriptPath;  // .as, .lua, .js
    QString className;     // entry class for AS3
    QMap<QString, QJsonValue> properties;  // exposed properties
    bool autoStart = true;

    ScriptComponent() { type = registerComponentType("Script"); }
    Component* clone() const override;
};

//------------------------------------------------------------------
// AudioComponent - sound emitter
//------------------------------------------------------------------
struct DVAPI AudioComponent : Component {
    TFilePath soundPath;
    bool loop = false;
    bool playOnStart = false;
    double volume = 1.0;
    double pitch = 1.0;
    bool spatial = false;  // 3D audio
    double maxDistance = 100.0;
    double rolloffFactor = 1.0;

    // Runtime
    void* soundHandle = nullptr;

    AudioComponent() { type = registerComponentType("Audio"); }
    Component* clone() const override;
};

//------------------------------------------------------------------
// CameraComponent - viewport/camera
//------------------------------------------------------------------
struct DVAPI CameraComponent : Component {
    double viewportWidth = 800, viewportHeight = 600;
    double zoom = 1.0;
    bool followEntity = false;
    EntityID followTarget = INVALID_ENTITY;
    double lerpSpeed = 5.0;  // camera smoothing
    QRectF bounds;  // camera bounds (empty = unlimited)

    CameraComponent() { type = registerComponentType("Camera"); }
    Component* clone() const override;
};

//------------------------------------------------------------------
// UIComponent - UI elements
//------------------------------------------------------------------
struct DVAPI UIComponent : Component {
    enum UIType { Button, Text, Image, ProgressBar, Slider, InputField, Panel };
    UIType uiType = Button;
    QString text;
    TFilePath imagePath;
    QRectF rect;
    QString fontFamily;
    int fontSize = 16;
    QRgb color = 0xFFFFFFFF;
    QString alignment = "center";
    bool interactive = true;
    QString onClickScript;  // script to run on click

    UIComponent() { type = registerComponentType("UI"); }
    Component* clone() const override;
};

//------------------------------------------------------------------
// ParticleComponent - particle system
//------------------------------------------------------------------
struct DVAPI ParticleComponent : Component {
    TFilePath emitterConfig;  // .pex or custom JSON
    bool autoPlay = true;
    bool loop = true;
    double duration = -1;  // -1 = infinite
    double emissionRate = 10;
    double lifeTime = 1.0;
    double startSize = 10, endSize = 0;
    QRgb startColor = 0xFFFFFFFF, endColor = 0x00FFFFFF;
    double gravity = 0;

    // Runtime
    void* emitterHandle = nullptr;

    ParticleComponent() { type = registerComponentType("Particle"); }
    Component* clone() const override;
};

//------------------------------------------------------------------
// Entity - just an ID with component management
//------------------------------------------------------------------
class DVAPI Entity {
public:
    Entity() = default;
    explicit Entity(EntityID id) : id_(id) {}

    EntityID id() const { return id_; }
    bool valid() const { return id_ != INVALID_ENTITY; }
    EntityMask mask() const { return mask_; }

    template<typename T>
    T* getComponent() {
        auto it = components_.find(T::staticType());
        return it != components_.end() ? static_cast<T*>(it->second) : nullptr;
    }

    template<typename T>
    const T* getComponent() const {
        auto it = components_.find(T::staticType());
        return it != components_.end() ? static_cast<T*>(it->second) : nullptr;
    }

    template<typename T, typename... Args>
    T* addComponent(Args&&... args) {
        static_assert(std::is_base_of<Component, T>::value, "T must derive from Component");
        T* comp = new T(std::forward<Args>(args)...);
        comp->entity = id_;
        components_[T::staticType()] = comp;
        mask_ |= (EntityMask(1) << T::staticType());
        return comp;
    }

    template<typename T>
    void removeComponent() {
        auto it = components_.find(T::staticType());
        if (it != components_.end()) {
            delete it->second;
            components_.erase(it);
            mask_ &= ~(EntityMask(1) << T::staticType());
        }
    }

    bool hasComponent(ComponentID type) const {
        return components_.find(type) != components_.end();
    }

    bool operator==(const Entity& other) const { return id_ == other.id_; }
    bool operator!=(const Entity& other) const { return id_ != other.id_; }

private:
    friend class Scene;
    EntityID id_ = INVALID_ENTITY;
    EntityMask mask_ = 0;
    QMap<ComponentID, Component*> components_;
};

// Static type IDs for built-in components
template<> constexpr ComponentID TransformComponent::staticType = 1;
template<> constexpr ComponentID SpriteComponent::staticType = 2;
template<> constexpr ComponentID PhysicsBodyComponent::staticType = 3;
template<> constexpr ComponentID ScriptComponent::staticType = 4;
template<> constexpr ComponentID AudioComponent::staticType = 5;
template<> constexpr ComponentID CameraComponent::staticType = 6;
template<> constexpr ComponentID UIComponent::staticType = 7;
template<> constexpr ComponentID ParticleComponent::staticType = 8;

//------------------------------------------------------------------
// System - logic operating on entities
//------------------------------------------------------------------
class DVAPI System {
public:
    virtual ~System() = default;

    virtual void init(Scene* scene) {}
    virtual void update(double dt) = 0;
    virtual void render() {}
    virtual void onEntityAdded(Entity* entity) {}
    virtual void onEntityRemoved(Entity* entity) {}

    EntityMask requiredMask() const { return requiredMask_; }
    void setRequiredMask(EntityMask mask) { requiredMask_ = mask; }

    Scene* scene() const { return scene_; }
    void setScene(Scene* s) { scene_ = s; }

    int priority() const { return priority_; }
    void setPriority(int p) { priority_ = p; }

protected:
    EntityMask requiredMask_ = 0;
    Scene* scene_ = nullptr;
    int priority_ = 0;
};

//------------------------------------------------------------------
// Built-in Systems
//------------------------------------------------------------------

// TransformSystem - updates world transforms from local transforms
class DVAPI TransformSystem : public System {
public:
    TransformSystem();
    void update(double dt) override;
};

// SpriteSystem - renders sprites
class DVAPI SpriteSystem : public System {
public:
    SpriteSystem();
    void update(double dt) override;
    void render() override;
};

// PhysicsSystem - Box2D simulation
class DVAPI PhysicsSystem : public System {
public:
    PhysicsSystem();
    ~PhysicsSystem();
    void init(Scene* scene) override;
    void update(double dt) override;
    void setGravity(double x, double y);
    void setWorldScale(double pixelsPerMeter);

    // Collision callbacks
    using CollisionCallback = std::function<void(EntityID a, EntityID b, bool begin)>;
    void setCollisionCallback(CollisionCallback cb);

private:
    void* world_ = nullptr;  // b2World*
    double worldScale_ = 30.0;  // pixels per meter
    CollisionCallback collisionCallback_;
};

// ScriptSystem - AS3/Lua/JS execution
class DVAPI ScriptSystem : public System {
public:
    ScriptSystem();
    void init(Scene* scene) override;
    void update(double dt) override;
    void loadScript(const TFilePath& path);
    void callFunction(const QString& funcName, const QVector<QJsonValue>& args);

private:
    void* scriptEngine_ = nullptr;  // Lua/JS/AVM2 context
};

// AudioSystem - sound playback
class DVAPI AudioSystem : public System {
public:
    AudioSystem();
    void init(Scene* scene) override;
    void update(double dt) override;
    void playSound(const TFilePath& path, double volume = 1.0, bool loop = false);
    void stopAll();
    void setMasterVolume(double v);

private:
    double masterVolume_ = 1.0;
};

// CameraSystem - viewport management
class DVAPI CameraSystem : public System {
public:
    CameraSystem();
    void update(double dt) override;
    void setViewport(double w, double h);

    // Get world-to-screen and screen-to-world transforms
    QTransform worldToScreen() const;
    QTransform screenToWorld() const;
    QPointF screenToWorld(const QPointF& screenPos) const;
    QPointF worldToScreen(const QPointF& worldPos) const;
};

// ParticleSystem - particle effects
class DVAPI ParticleSystem : public System {
public:
    ParticleSystem();
    void update(double dt) override;
    void render() override;
    void* createEmitter(const TFilePath& config, double x, double y);
    void destroyEmitter(void* handle);
};

// UISystem - UI rendering and input
class DVAPI UISystem : public System {
public:
    UISystem();
    void update(double dt) override;
    void render() override;
    void handleMouseEvent(const QPointF& pos, int button, bool pressed);
    void handleKeyEvent(int key, bool pressed);
    void handleTextInput(const QString& text);
};

//------------------------------------------------------------------
// Scene - owns entities and systems
//------------------------------------------------------------------
class DVAPI Scene {
public:
    Scene(const QString& name = "Scene");
    ~Scene();

    const QString& name() const { return name_; }
    void setName(const QString& n) { name_ = n; }

    // Entity management
    Entity createEntity();
    void destroyEntity(EntityID id);
    Entity* getEntity(EntityID id);
    const Entity* getEntity(EntityID id) const;

    // Query entities with component mask
    QVector<Entity*> query(EntityMask mask);
    template<typename... Components>
    QVector<Entity*> query() {
        EntityMask mask = 0;
        (mask |= (EntityMask(1) << Components::staticType), ...);
        return query(mask);
    }

    // System management
    void addSystem(System* system);
    void removeSystem(System* system);
    System* getSystem(SystemID type);

    // Lifecycle
    void init();
    void update(double dt);
    void render();
    void shutdown();

    // Asset loading
    AssetManager* assets() { return assets_.get(); }

    // Input
    InputManager* input() { return input_.get(); }

    // Physics
    PhysicsWorld* physics() { return physics_.get(); }

    // Audio
    AudioManager* audio() { return audio_.get(); }

    // Serialize/deserialize
    bool save(const TFilePath& path);
    bool load(const TFilePath& path);

private:
    QString name_;
    EntityID nextEntityID_ = 1;
    QMap<EntityID, Entity> entities_;
    QVector<System*> systems_;
    SystemID nextSystemID_ = 1;

    std::unique_ptr<AssetManager> assets_;
    std::unique_ptr<InputManager> input_;
    std::unique_ptr<PhysicsWorld> physics_;
    std::unique_ptr<AudioManager> audio_;
};

//------------------------------------------------------------------
// AssetManager - loads game assets via Flare's IO
//------------------------------------------------------------------
class DVAPI AssetManager {
public:
    AssetManager();
    ~AssetManager();

    // Load various asset types
    // Returns handle (opaque pointer), caller owns it
    void* loadTexture(const TFilePath& path);
    void* loadSWFSymbol(const TFilePath& swfPath, const QString& symbolName);
    void* loadAudio(const TFilePath& path);
    void* loadScript(const TFilePath& path);
    void* loadParticleConfig(const TFilePath& path);
    void* loadFont(const TFilePath& path);

    // Release
    void release(void* handle);
    void releaseAll();

    // Cache management
    void setCacheSize(size_t bytes);
    size_t cacheSize() const;

private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};

//------------------------------------------------------------------
// InputManager - keyboard, mouse, touch, gamepad
//------------------------------------------------------------------
class DVAPI InputManager {
public:
    InputManager();
    ~InputManager();

    void update();

    bool isKeyPressed(int key) const;
    bool isKeyJustPressed(int key) const;
    bool isKeyJustReleased(int key) const;

    bool isMouseButtonPressed(int button) const;
    bool isMouseButtonJustPressed(int button) const;
    QPointF mousePosition() const;
    QPointF mouseDelta() const;

    bool isGamepadConnected(int index) const;
    float gamepadAxis(int index, int axis) const;
    bool gamepadButton(int index, int button) const;

    // Text input
    QString getTextInput() const;
    void clearTextInput();

    // Key constants (Qt-compatible)
    static constexpr int Key_Unknown = 0;
    static constexpr int Key_Escape = 0x01000000;
    static constexpr int Key_Left = 0x01000012;
    static constexpr int Key_Right = 0x01000014;
    static constexpr int Key_Up = 0x01000013;
    static constexpr int Key_Down = 0x01000015;
    static constexpr int Key_Space = 0x20;
    static constexpr int Key_Enter = 0x01000004;
    static constexpr int Key_W = 0x57;
    static constexpr int Key_A = 0x41;
    static constexpr int Key_S = 0x53;
    static constexpr int Key_D = 0x44;
};

}  // namespace TGame

#endif  // TGAME_GAMEENGINE_H_