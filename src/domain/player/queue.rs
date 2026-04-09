use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::QueueItem;

/// 纯内存播放队列，集中封装索引修正规则。
#[derive(Debug, Clone, Default)]
pub struct PlayerQueue {
    items: Vec<QueueItem>,
    current_index: Option<usize>,
}

impl PlayerQueue {
    /// 使用给定队列项和当前索引构造播放器队列。
    ///
    /// # 参数
    /// - `items`：初始队列项
    /// - `current_index`：初始当前索引；越界时会被自动丢弃
    ///
    /// # 返回值
    /// - `Self`：构造后的播放队列
    pub fn from_items(items: Vec<QueueItem>, current_index: Option<usize>) -> Self {
        let current_index = current_index.filter(|index| *index < items.len());
        Self {
            items,
            current_index,
        }
    }

    /// 返回当前队列长度。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `usize`：队列项数量
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 判断当前队列是否为空。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `bool`：队列是否为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 返回只读队列视图。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&[QueueItem]`：队列项切片
    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    /// 返回当前索引。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<usize>`：当前索引
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// 返回当前播放项。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<&QueueItem>`：当前队列项
    pub fn current(&self) -> Option<&QueueItem> {
        self.current_index.and_then(|index| self.items.get(index))
    }

    /// 追加一个队列项到尾部。
    ///
    /// # 参数
    /// - `item`：待追加的队列项
    ///
    /// # 返回值
    /// - 无
    pub fn append(&mut self, item: QueueItem) {
        self.items.push(item);
    }

    /// 在指定位置插入队列项，并在必要时前移当前索引。
    ///
    /// # 参数
    /// - `index`：插入位置
    /// - `item`：待插入的队列项
    ///
    /// # 返回值
    /// - `MeloResult<()>`：插入结果
    pub fn insert(&mut self, index: usize, item: QueueItem) -> MeloResult<()> {
        if index > self.items.len() {
            return Err(MeloError::Message("queue index out of range".to_string()));
        }

        self.items.insert(index, item);
        if let Some(current_index) = self.current_index
            && index <= current_index
        {
            self.current_index = Some(current_index + 1);
        }

        Ok(())
    }

    /// 选择指定索引作为当前播放项。
    ///
    /// # 参数
    /// - `index`：目标索引
    ///
    /// # 返回值
    /// - `MeloResult<&QueueItem>`：当前被选中的队列项
    pub fn play_index(&mut self, index: usize) -> MeloResult<&QueueItem> {
        if index >= self.items.len() {
            return Err(MeloError::Message("queue index out of range".to_string()));
        }

        self.current_index = Some(index);
        Ok(&self.items[index])
    }

    /// 删除指定位置的队列项，并按约定修复当前索引。
    ///
    /// # 参数
    /// - `index`：待删除索引
    ///
    /// # 返回值
    /// - `MeloResult<Option<QueueItem>>`：被删除的队列项
    pub fn remove(&mut self, index: usize) -> MeloResult<Option<QueueItem>> {
        if index >= self.items.len() {
            return Err(MeloError::Message("queue index out of range".to_string()));
        }

        let removed = self.items.remove(index);
        self.current_index = match self.current_index {
            None => None,
            Some(_) if self.items.is_empty() => None,
            Some(current) if index < current => Some(current - 1),
            Some(current) if index == current && index < self.items.len() => Some(index),
            Some(current) if index == current => Some(current.saturating_sub(1)),
            Some(current) => Some(current),
        };

        Ok(Some(removed))
    }

    /// 移动指定队列项，并同步修复当前索引。
    ///
    /// # 参数
    /// - `from`：源索引
    /// - `to`：目标索引
    ///
    /// # 返回值
    /// - `MeloResult<()>`：移动结果
    pub fn move_item(&mut self, from: usize, to: usize) -> MeloResult<()> {
        if from >= self.items.len() || to >= self.items.len() {
            return Err(MeloError::Message("queue index out of range".to_string()));
        }
        if from == to {
            return Ok(());
        }

        let item = self.items.remove(from);
        self.items.insert(to, item);

        self.current_index = match self.current_index {
            Some(current) if current == from => Some(to),
            Some(current) if from < current && to >= current => Some(current - 1),
            Some(current) if from > current && to <= current => Some(current + 1),
            other => other,
        };

        Ok(())
    }

    /// 清空整个队列，并丢弃当前索引。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
    }

    /// 判断当前是否存在下一首。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `bool`：是否存在下一首
    pub fn has_next(&self) -> bool {
        matches!(self.current_index, Some(index) if index + 1 < self.items.len())
    }

    /// 判断当前是否存在上一首。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `bool`：是否存在上一首
    pub fn has_prev(&self) -> bool {
        matches!(self.current_index, Some(index) if index > 0)
    }
}

#[cfg(test)]
mod tests;
