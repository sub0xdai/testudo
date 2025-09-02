use leptos::prelude::*;
use leptos::task::spawn_local;
use thaw::{Card, Space, Flex, Button, ButtonSize, Icon, FlexAlign, FlexGap, SpaceGap};
use icondata as i;
use gloo_timers::future::TimeoutFuture;

#[derive(Clone, Debug, PartialEq)]
pub enum NotificationType {
    Success,
    Error,
    Warning,
    Info,
}

impl NotificationType {
    pub fn icon(&self) -> icondata_core::Icon {
        match self {
            NotificationType::Success => i::AiCheckCircleOutlined,
            NotificationType::Error => i::AiCloseCircleOutlined,
            NotificationType::Warning => i::AiExclamationCircleOutlined,
            NotificationType::Info => i::AiInfoCircleOutlined,
        }
    }
    
    pub fn class(&self) -> &'static str {
        match self {
            NotificationType::Success => "thaw-message--success",
            NotificationType::Error => "thaw-message--error", 
            NotificationType::Warning => "thaw-message--warning",
            NotificationType::Info => "thaw-message--info",
        }
    }
    
    pub fn color(&self) -> &'static str {
        match self {
            NotificationType::Success => "text-green-400",
            NotificationType::Error => "text-red-400",
            NotificationType::Warning => "text-orange-400", 
            NotificationType::Info => "text-roman-gold",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub message: String,
    pub notification_type: NotificationType,
    pub created_at: f64, // timestamp
    pub duration: Option<f64>, // auto-dismiss after duration (ms)
}

impl Notification {
    pub fn new(
        title: String,
        message: String,
        notification_type: NotificationType,
        duration: Option<f64>,
    ) -> Self {
        let id = format!("notification_{}", js_sys::Date::now());
        let created_at = js_sys::Date::now();
        
        Self {
            id,
            title,
            message,
            notification_type,
            created_at,
            duration,
        }
    }
    
    pub fn success(title: String, message: String) -> Self {
        Self::new(title, message, NotificationType::Success, Some(5000.0))
    }
    
    pub fn error(title: String, message: String) -> Self {
        Self::new(title, message, NotificationType::Error, Some(10000.0))
    }
    
    pub fn warning(title: String, message: String) -> Self {
        Self::new(title, message, NotificationType::Warning, Some(7000.0))
    }
    
    pub fn info(title: String, message: String) -> Self {
        Self::new(title, message, NotificationType::Info, Some(5000.0))
    }
}

#[component]
pub fn NotificationSystem() -> impl IntoView {
    let (notifications, set_notifications) = signal::<Vec<Notification>>(vec![]);
    
    // Provide notification functions to child components
    provide_context(NotificationProvider {
        show: Callback::new(move |notification: Notification| {
            set_notifications.update(|n| n.push(notification));
        }),
        remove: Callback::new({
            let set_notifications = set_notifications;
            move |id: String| {
                set_notifications.update(|n| n.retain(|notification| notification.id != id));
            }
        }),
    });
    
    view! {
        <div class="fixed top-4 right-4 z-50 space-y-2">
            <For
                each=move || notifications.get()
                key=|notification| notification.id.clone()
                children=move |notification: Notification| {
                    view! {
                        <NotificationItem 
                            notification=notification
                            on_close=Callback::new({
                                let set_notifications = set_notifications;
                                move |id: String| {
                                    set_notifications.update(|n| n.retain(|n| n.id != id));
                                }
                            })
                        />
                    }
                }
            />
        </div>
    }
}

#[derive(Clone)]
pub struct NotificationProvider {
    pub show: Callback<Notification>,
    pub remove: Callback<String>,
}

// Hook for using notifications
pub fn use_notification() -> NotificationProvider {
    use_context::<NotificationProvider>().expect("NotificationProvider not found")
}

#[component]
fn NotificationItem(
    #[prop(into)] notification: Notification,
    #[prop(into)] on_close: Callback<String>,
) -> impl IntoView {
    let (is_visible, set_is_visible) = signal(true);
    
    // Auto-dismiss timer
    if let Some(duration) = notification.duration {
        let notification_id = notification.id.clone();
        let on_close_timer = on_close;
        
        spawn_local(async move {
            TimeoutFuture::new(duration as u32).await;
            on_close_timer(notification_id);
        });
    }
    
    let close_notification = {
        let id = notification.id.clone();
        move || {
            set_is_visible.set(false);
            // Small delay for exit animation
            let id_clone = id.clone();
            let on_close_delayed = on_close;
            spawn_local(async move {
                TimeoutFuture::new(200).await;
                on_close_delayed(id_clone);
            });
        }
    };
    
    view! {
        <Show when=move || is_visible.get()>
            <Card class=format!(
                "thaw-message {} min-w-72 max-w-96 notification-enter", 
                notification.notification_type.class()
            )>
                <div class="p-4">
                    <Flex gap=FlexGap::Medium align=FlexAlign::Start>
                        <Icon 
                            icon=notification.notification_type.icon()
                            class=format!("text-lg {}", notification.notification_type.color())
                        />
                        <div class="flex-1">
                            <Space vertical=true gap=SpaceGap::Small>
                                <div class="font-semibold">{notification.title}</div>
                                <div class="text-sm text-gray-300">{notification.message}</div>
                            </Space>
                        </div>
                        <Button
                            size=ButtonSize::Small
                            class="hover:text-gray-300"
                            on_click=move |_| close_notification()
                        >
                            <Icon icon=i::AiCloseOutlined />
                        </Button>
                    </Flex>
                </div>
            </Card>
        </Show>
    }
}

// Trading-specific notification helpers
pub struct TradingNotifications;

impl TradingNotifications {
    pub fn order_executed(symbol: String, side: String, amount: f64) -> Notification {
        Notification::success(
            "Order Executed".to_string(),
            format!("{} {} {} successfully executed", side.to_uppercase(), amount, symbol),
        )
    }
    
    pub fn order_failed(symbol: String, error: String) -> Notification {
        Notification::error(
            "Order Failed".to_string(),
            format!("Failed to place {} order: {}", symbol, error),
        )
    }
    
    pub fn risk_warning(message: String) -> Notification {
        Notification::warning(
            "Risk Warning".to_string(),
            message,
        )
    }
    
    pub fn connection_lost() -> Notification {
        Notification::error(
            "Connection Lost".to_string(),
            "Lost connection to trading servers. Attempting to reconnect...".to_string(),
        )
    }
    
    pub fn connection_restored() -> Notification {
        Notification::success(
            "Connection Restored".to_string(),
            "Successfully reconnected to trading servers.".to_string(),
        )
    }
    
    pub fn van_tharp_alert(message: String) -> Notification {
        Notification::warning(
            "Van Tharp Risk Assessment".to_string(),
            message,
        )
    }
}