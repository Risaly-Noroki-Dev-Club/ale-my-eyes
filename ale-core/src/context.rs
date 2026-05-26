use crate::cloud::CloudMessage;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// 上下文条目角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
}

/// 上下文条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    pub role: Role,
    pub content: String,
    pub image_ref: Option<String>,
    pub token_count: usize,
}

/// 帧摘要（存储 AI 生成的描述，而非原始图像）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameSummary {
    pub description: String,
    pub key_elements: Vec<String>,
    pub source: FrameSource,
}

/// 帧来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameSource {
    Camera,
    Screen,
}

/// 长期记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub content: String,
    pub importance: f32,
    pub source: String,
}

/// 上下文管理器
pub struct ContextManager {
    /// 对话历史
    conversation: VecDeque<ContextEntry>,
    /// 视觉记忆（最近帧的摘要）
    visual_memory: VecDeque<FrameSummary>,
    /// 最大视觉记忆条目数
    max_visual: usize,
    /// 长期记忆
    long_term_memory: Vec<MemoryEntry>,
    /// 对话摘要（压缩后的旧对话）
    conversation_summary: Option<String>,
    /// 当前 token 估算
    current_tokens: usize,
    /// 最大 token 预算
    max_tokens: usize,
    /// 系统提示
    system_prompt: String,
}

impl ContextManager {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            conversation: VecDeque::new(),
            visual_memory: VecDeque::new(),
            max_visual: 10,
            long_term_memory: Vec::new(),
            conversation_summary: None,
            current_tokens: 0,
            max_tokens,
            system_prompt: Self::default_system_prompt(),
        }
    }

    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = prompt;
        self
    }

    fn default_system_prompt() -> String {
        "你是 Ale, My Eyes! 智能视觉辅助助手。你可以看到用户的摄像头画面或电脑屏幕，帮助用户理解视觉内容并执行操作。请用简洁自然的中文回答。".to_string()
    }

    /// 添加用户消息
    pub fn add_user_message(&mut self, content: String) {
        let token_count = self.estimate_tokens(&content);
        self.conversation.push_back(ContextEntry {
            role: Role::User,
            content,
            image_ref: None,
            token_count,
        });
        self.current_tokens += token_count;
        self.maybe_compact();
    }

    /// 添加助手消息
    pub fn add_assistant_message(&mut self, content: String) {
        let token_count = self.estimate_tokens(&content);
        self.conversation.push_back(ContextEntry {
            role: Role::Assistant,
            content,
            image_ref: None,
            token_count,
        });
        self.current_tokens += token_count;
        self.maybe_compact();
    }

    /// 添加带图像引用的用户消息
    pub fn add_user_message_with_image(&mut self, content: String, image_ref: String) {
        let token_count = self.estimate_tokens(&content) + 100; // 图像描述的 token 估算
        self.conversation.push_back(ContextEntry {
            role: Role::User,
            content,
            image_ref: Some(image_ref),
            token_count,
        });
        self.current_tokens += token_count;
        self.maybe_compact();
    }

    /// 添加视觉帧摘要
    pub fn add_frame_summary(&mut self, summary: FrameSummary) {
        // 检查是否与最近帧重复
        if let Some(last) = self.visual_memory.back() {
            if last.description == summary.description {
                return;
            }
        }

        self.visual_memory.push_back(summary);
        if self.visual_memory.len() > self.max_visual {
            self.visual_memory.pop_front();
        }
    }

    /// 添加长期记忆
    pub fn add_memory(&mut self, entry: MemoryEntry) {
        // 检查是否已存在
        if self
            .long_term_memory
            .iter()
            .any(|m| m.content == entry.content)
        {
            return;
        }
        self.long_term_memory.push(entry);
    }

    /// 构建发送给 AI 的消息列表
    pub fn build_messages(
        &self,
        current_image_description: Option<&str>,
        current_question: &str,
    ) -> Vec<CloudMessage> {
        let mut messages = Vec::new();

        // 1. 系统提示（含长期记忆）
        let mut system = self.system_prompt.clone();
        if !self.long_term_memory.is_empty() {
            system.push_str("\n\n用户相关记忆：\n");
            for mem in &self.long_term_memory {
                system.push_str(&format!("- {}\n", mem.content));
            }
        }
        messages.push(CloudMessage {
            role: "system".to_string(),
            content: system,
        });

        // 2. 视觉上下文（最近帧摘要）
        if !self.visual_memory.is_empty() {
            let mut visual_context = String::from("最近的画面变化：\n");
            for (i, frame) in self.visual_memory.iter().rev().take(3).enumerate() {
                visual_context.push_str(&format!(
                    "{}. [{}] {}\n",
                    i + 1,
                    match frame.source {
                        FrameSource::Camera => "相机",
                        FrameSource::Screen => "屏幕",
                    },
                    frame.description
                ));
                if !frame.key_elements.is_empty() {
                    visual_context
                        .push_str(&format!("   关键元素: {}\n", frame.key_elements.join(", ")));
                }
            }
            messages.push(CloudMessage {
                role: "system".to_string(),
                content: visual_context,
            });
        }

        // 3. 对话摘要
        if let Some(ref summary) = self.conversation_summary {
            messages.push(CloudMessage {
                role: "system".to_string(),
                content: format!("之前的对话摘要：{}", summary),
            });
        }

        // 4. 最近对话
        for entry in self.conversation.iter().rev().take(10).rev() {
            let role = match entry.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
            };
            messages.push(CloudMessage {
                role: role.to_string(),
                content: entry.content.clone(),
            });
        }

        // 5. 当前问题 + 当前图像描述
        let mut user_content = current_question.to_string();
        if let Some(img_desc) = current_image_description {
            user_content = format!("[当前画面：{}]\n\n{}", img_desc, current_question);
        }
        messages.push(CloudMessage {
            role: "user".to_string(),
            content: user_content,
        });

        messages
    }

    /// 格式化视觉记忆（用于调试）
    pub fn format_visual_memory(&self) -> String {
        self.visual_memory
            .iter()
            .map(|f| format!("[{:?}] {}", f.source, f.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 压缩对话（当 token 接近上限时）
    fn maybe_compact(&mut self) {
        if self.current_tokens < self.max_tokens * 80 / 100 {
            return;
        }

        // 将前半部分对话压缩为摘要
        let half = self.conversation.len() / 2;
        if half < 3 {
            return;
        }

        let mut summary_parts = Vec::new();
        for _ in 0..half {
            if let Some(entry) = self.conversation.pop_front() {
                self.current_tokens = self.current_tokens.saturating_sub(entry.token_count);
                if entry.role == Role::User || entry.role == Role::Assistant {
                    summary_parts.push(format!(
                        "[{}]: {}",
                        match entry.role {
                            Role::User => "用户",
                            Role::Assistant => "助手",
                            _ => "系统",
                        },
                        // 截断过长的内容
                        if entry.content.len() > 100 {
                            format!("{}...", &entry.content[..100])
                        } else {
                            entry.content
                        }
                    ));
                }
            }
        }

        let new_summary = if let Some(existing) = &self.conversation_summary {
            format!("{}\n{}", existing, summary_parts.join("\n"))
        } else {
            summary_parts.join("\n")
        };

        // 进一步压缩：如果摘要也太长，只保留最后部分
        self.conversation_summary = Some(if new_summary.len() > 2000 {
            format!("...{}", &new_summary[new_summary.len() - 2000..])
        } else {
            new_summary
        });
    }

    /// 简单的 token 估算（1 中文字符 ≈ 2 tokens，1 英文单词 ≈ 1 token）
    fn estimate_tokens(&self, text: &str) -> usize {
        let chinese_chars = text.chars().filter(|c| *c as u32 > 0x4E00).count();
        let other_chars = text.len() - chinese_chars;
        chinese_chars * 2 + other_chars / 4
    }

    /// 清空上下文
    pub fn clear(&mut self) {
        self.conversation.clear();
        self.visual_memory.clear();
        self.conversation_summary = None;
        self.current_tokens = 0;
    }

    /// 获取当前对话轮数
    pub fn conversation_turns(&self) -> usize {
        self.conversation.len()
    }

    /// 获取当前 token 估算
    pub fn estimated_tokens(&self) -> usize {
        self.current_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_manager_basic() {
        let mut ctx = ContextManager::new(4000);
        ctx.add_user_message("你好".to_string());
        ctx.add_assistant_message("你好！有什么可以帮助你的？".to_string());

        assert_eq!(ctx.conversation_turns(), 2);
        assert!(ctx.estimated_tokens() > 0);
    }

    #[test]
    fn test_build_messages() {
        let mut ctx = ContextManager::new(4000);
        ctx.add_user_message("这是什么？".to_string());
        ctx.add_assistant_message("这是一只猫。".to_string());

        let messages = ctx.build_messages(Some("一只橘色的猫"), "它在做什么？");
        assert!(messages.len() >= 3); // system + history + current
        assert!(messages.last().unwrap().content.contains("橘色的猫"));
    }

    #[test]
    fn test_visual_memory() {
        let mut ctx = ContextManager::new(4000);
        ctx.add_frame_summary(FrameSummary {
            description: "桌面环境".to_string(),
            key_elements: vec!["任务栏".to_string(), "浏览器窗口".to_string()],
            source: FrameSource::Screen,
        });

        assert_eq!(ctx.visual_memory.len(), 1);

        // 重复帧不添加
        ctx.add_frame_summary(FrameSummary {
            description: "桌面环境".to_string(),
            key_elements: vec!["任务栏".to_string()],
            source: FrameSource::Screen,
        });
        assert_eq!(ctx.visual_memory.len(), 1);
    }

    #[test]
    fn test_compact() {
        let mut ctx = ContextManager::new(100); // 很小的 token 预算
        for i in 0..20 {
            ctx.add_user_message(format!("消息 {}", i));
            ctx.add_assistant_message(format!("回复 {}", i));
        }

        // 应该触发压缩
        assert!(ctx.conversation_summary.is_some());
        assert!(ctx.conversation_turns() < 40);
    }

    #[test]
    fn test_memory() {
        let mut ctx = ContextManager::new(4000);
        ctx.add_memory(MemoryEntry {
            content: "用户喜欢简洁的回答".to_string(),
            importance: 0.8,
            source: "对话".to_string(),
        });

        // 重复记忆不添加
        ctx.add_memory(MemoryEntry {
            content: "用户喜欢简洁的回答".to_string(),
            importance: 0.9,
            source: "对话".to_string(),
        });
        assert_eq!(ctx.long_term_memory.len(), 1);
    }

    #[test]
    fn test_clear() {
        let mut ctx = ContextManager::new(4000);
        ctx.add_user_message("test".to_string());
        ctx.add_frame_summary(FrameSummary {
            description: "test".to_string(),
            key_elements: vec![],
            source: FrameSource::Camera,
        });

        ctx.clear();
        assert_eq!(ctx.conversation_turns(), 0);
        assert_eq!(ctx.visual_memory.len(), 0);
    }
}
