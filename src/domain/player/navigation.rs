use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::model::player::RepeatMode;

/// 播放导航规则计算器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackNavigation {
    visible_len: usize,
    current_visible_index: Option<usize>,
    order: Vec<usize>,
}

impl PlaybackNavigation {
    /// 构造线性播放顺序的导航器。
    ///
    /// # 参数
    /// - `visible_len`：可见队列长度
    /// - `current_visible_index`：当前可见索引
    ///
    /// # 返回值
    /// - `Self`：线性导航器
    pub fn linear(visible_len: usize, current_visible_index: Option<usize>) -> Self {
        Self {
            visible_len,
            current_visible_index,
            order: (0..visible_len).collect(),
        }
    }

    /// 构造带稳定随机顺序的导航器。
    ///
    /// # 参数
    /// - `visible_len`：可见队列长度
    /// - `current_visible_index`：当前可见索引
    /// - `seed`：随机顺序种子
    ///
    /// # 返回值
    /// - `Self`：随机导航器
    pub fn shuffled(visible_len: usize, current_visible_index: Option<usize>, seed: u64) -> Self {
        let mut order = (0..visible_len).collect::<Vec<_>>();
        order.sort_by_key(|index| {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            index.hash(&mut hasher);
            hasher.finish()
        });
        Self {
            visible_len,
            current_visible_index,
            order,
        }
    }

    /// 返回当前投影顺序。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&[usize]`：当前投影顺序
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// 返回当前可见索引。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<usize>`：当前可见索引
    pub fn current_visible_index(&self) -> Option<usize> {
        self.current_visible_index
    }

    /// 计算“下一首”的索引。
    ///
    /// # 参数
    /// - `repeat_mode`：循环模式
    /// - `shuffle_enabled`：是否启用随机顺序
    ///
    /// # 返回值
    /// - `Option<usize>`：下一首索引
    pub fn next_index(&self, repeat_mode: RepeatMode, shuffle_enabled: bool) -> Option<usize> {
        self.advance(Direction::Next, repeat_mode, shuffle_enabled)
    }

    /// 计算“上一首”的索引。
    ///
    /// # 参数
    /// - `repeat_mode`：循环模式
    /// - `shuffle_enabled`：是否启用随机顺序
    ///
    /// # 返回值
    /// - `Option<usize>`：上一首索引
    pub fn prev_index(&self, repeat_mode: RepeatMode, shuffle_enabled: bool) -> Option<usize> {
        self.advance(Direction::Prev, repeat_mode, shuffle_enabled)
    }

    /// 计算自然播完后的目标索引。
    ///
    /// # 参数
    /// - `repeat_mode`：循环模式
    /// - `shuffle_enabled`：是否启用随机顺序
    ///
    /// # 返回值
    /// - `Option<usize>`：播完后应跳转的索引
    pub fn track_end_index(&self, repeat_mode: RepeatMode, shuffle_enabled: bool) -> Option<usize> {
        if repeat_mode == RepeatMode::One {
            return self.current_visible_index;
        }
        self.advance(Direction::Next, repeat_mode, shuffle_enabled)
    }

    /// 根据方向和模式推进索引。
    ///
    /// # 参数
    /// - `direction`：推进方向
    /// - `repeat_mode`：循环模式
    /// - `shuffle_enabled`：是否启用随机顺序
    ///
    /// # 返回值
    /// - `Option<usize>`：推进后的索引
    fn advance(
        &self,
        direction: Direction,
        repeat_mode: RepeatMode,
        shuffle_enabled: bool,
    ) -> Option<usize> {
        let current = self.current_visible_index?;
        let linear_order;
        let order: &[usize] = if shuffle_enabled {
            &self.order
        } else {
            linear_order = (0..self.visible_len).collect::<Vec<_>>();
            &linear_order
        };
        let order_pos = order.iter().position(|index| *index == current)?;

        match direction {
            Direction::Next if order_pos + 1 < order.len() => Some(order[order_pos + 1]),
            Direction::Prev if order_pos > 0 => Some(order[order_pos - 1]),
            Direction::Next if repeat_mode == RepeatMode::All => order.first().copied(),
            Direction::Prev if repeat_mode == RepeatMode::All => order.last().copied(),
            _ => None,
        }
    }
}

/// 播放导航方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Next,
    Prev,
}

#[cfg(test)]
mod tests;
